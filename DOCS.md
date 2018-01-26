
This document describes the language used by `paraphrase`. It is not complete at the moment,
but should suffice for most purposes. I refer the reader to the `tests` folder to see
more sophisticated demos.

# The Language

In paraphrase, every macro invocation starts with `#`; all text that is not part of such an invocation
is written to standard output. Macros can take one or more parameters in parentheses (`(...)`), 
brackets (`{...}`), or semicolons (`;...`); more information about this will be provided below.

## Data Types

In a macro invocation, every argument and return value has one of four possible data types:

* **Strings** consist of text. Using a string does not automatically expand it (unlike in some macro
  languages); strings are not expanded except on request.
* **Closures** consist of an (unexpanded) piece of code, along with a *scope* or namespace in which
  that code will be expanded.
* **Lists** consist of a sequence of values. Different items in the list can have different types,
  so that they can be used as tuples.
* **Tagged values** consist of a value together with a marker (or *tag*) indicating which command
  "tagged" them. This is used to implement custom data types.

## Parameter types

There are three ways to pass a parameter to a macro. Any of these mechanisms can be used to pass
values to any macro; the difference is only visible to the callee.

* **Parentheses**. If a macro parameter is passed in parentheses, the contents of the parentheses
  will be expanded *before* the macro is invoked. For instance, in `#h(#w)`, the macro `#w` will
  be invoked, and the resulting value will be passed to `#h`. In general, if the parentheses
  include multiple values, or values together with non-whitespace strings, then the values will
  be concatenated together; thus no macro for explicitly concatenating strings is necessary.
* **Brackets.** If a macro parameter is passed in brackets, the contents of the parentheses
  are treated as a closure with the current (lexical) scope. For instance, in `#h{#w}`, 
  the closure `{#w}` will be passed to `#h`, which can control how (or whether) this closure
  gets expanded.
* **Semicolons.** If a semicolon is used as a macro parameter, the entire rest of the enclosing
  block (i.e. the current closure, or the entire rest of the file) will be used as a parameter.
  The parameter will be passed as a closure. If necessary, paraphrase will suspend *parsing*
  the current block: this means that the macro can influence the parsing of future macros.
  Semicolons act like uniqueness types (a concept in some functional languages) to provide
  a way of explicitly ordering computations.

# Standard library

Below is a summary of the standard library provided by paraphrase. Each macro is listed
in *one* way that it might be used; for instance, `#define` can also be invoked as 
`#define(A){B}{C}`, but it is usually used with a semicolon as the third parameter.

### #define(NAME ARGS){CLOSURE};REST

The `#define` macro defines a new macro to be the contents of CLOSURE, and expands REST
with this macro being defined. If a non-closure value is supplied as CLOSURE,
it will be wrapped in a closure automatically.
`NAME ARGS` specifies the name of the macro and its parameters, along with their associated
types. For instance, in `#define(x y:string)...`, the macro's name is `#x`, it has one parameter
(named `y`), and this parameter will be required to be a `string`. `list`, `closure`, and `any` can also
be used as types; the user can also specify a user-defined type, as described below.

### #tag(VALUE)

The `#tag` macro adds a *tag*, associated with the currently-executing command, to a given value.
This tag can then be used as a data type for defining future macros.

For instance, if one writes `#define(point x:string y:string){#tag(#list(#x #y))};`, then
`#point(1)(2)` will produce a value of the `point` type; that is, the value
can be provided as input to `#define(distance a:point b:point)`.

### #untag{NAME}(VALUE)

The `#untag` macro *removes* the specified tag from a value. Note that one cannot remove the tag
from a given value without knowing what the tag is; this provides a limited form of encapsulation.
For instance, `#define(x_coordinate a:point){#head(#untag{point}(#a))}` will work (given the definition
of `point` above) to get a point's x-coordinate as a string.
Note that NAME must be provided as a closure, so that the current (lexical) scope can be used to look up the tag.

### #var_dump(VALUE)

The `#var_dump` macro displays the contents of the given value in a "pretty" format.
It is quite useful for debugging.

### #head(LIST), #tail(LIST)

The `#head` macro gets the first element of the list LIST; `#tail` gets everything except the last element.
This is analogous to `car` and `cdr` in Lisp-like languages.

### #join(A)(B)

The `#join` macro concatenates the lists A and B into a single list. It is useful for
writing recursive list-processing macros, much like `cons` in Lisp.

### #match_regex(REGEX,STRING)

Searches for the first occurence of REGEX in STRING, returing the values of all capturing groups
as a list. If REGEX is not found in STRING, an empty list is returned.

### #list(VALUE VALUE VALUE...)

The `#list` macro is used to build lists; for instance `#list(#literal{A} #literal{B})` creates a
list containing the string `A` and the string `B`. Note that adjacent values are not concatenated;
thus `#list` works differently from all other macros.

### #assert(MSG)(VAL_A)(VAL_B)

Asserts that VAL_A and VAL_B are equal, displaying MSG with a check mark or X-mark to indicate
success or failure. Used in the test suite.


### #if_eq(A)(B){TRUE}{FALSE}

If A and B are the same, TRUE is expanded; otherwise, FALSE will be expanded.
This is used as a simple conditional mechanism; more sophisticated conditionals will be provided
in a future version of the language. Since there is no real sensible way to compare closures,
closures are *never* treated as equal to each other.

### if_eq_then(A)(B){TRUE}{FALSE};FINALLY

The same as `#if_eq`, but runs FINALLY in the scope created by TRUE or FALSE. This can be used
to conditionally define macros. In most other cases, it can (and should) be replaced with `#if_eq`.

### #literal{STR}

Given a closure STR, this yields the contents of the closure; if STR is not a closure, it
returns its parameter unmodified. This is useful (for instance) in creating strings containing
special characters, or in adding strings to lists.


### #expand(CLOSURE)

Invokes CLOSURE in its associated scope, returning the results of that invocation.
This is used by macros which accept closures as arguments. Note that scope is *lexical*, not dynamic.

### #rescope{SCOPE_CLOSURE}(OTHER_CLOSURE)

Returns a closure with the *scope* of SCOPE_CLOSURE but the *contents* of OTHER_CLOSURE.
This allows the user to (for instance) make a lexically-scoped closure behave dynamically.



