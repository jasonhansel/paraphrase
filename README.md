# paraphrase

paraphrase is a concurrent, general-purpose macro preprocessor, with a role similar to
[m4](https://www.gnu.org/software/m4/m4.html).
You can use it to:

- **Add macros to any language.**
  paraphrase can expand macros in code written in any language, from COBOL to Haskell.
  It can thus replace language-specific tools like [sweet.js](https://www.sweetjs.org/),
  and make coding in older, less-sophisticated languages easier.
- **Write scripts focused on text processing.**
  paraphrase uses a Turing-complete scripting language to define and expand macros.
  It thus serves as a convenient interpreter for simple, text-focused tasks.
- **Perform simple templating.**
  paraphrase has basic support for JSON: if a JSON file is provided on the command line,
  it will be used to define macros available for use in the program itself. Thus paraphrase
  can replace other templating languages for simple tasks.

In particular, it focuses on:
- **Concurrency.**
  paraphrase is highly concurrent: macro expansion, unlike programming in general, can be
  parallellized in a fairly straightforward manner. paraphrases uses
  [futures-rs](https://github.com/alexcrichton/futures-rs) to manage a thread pool for this
  purpose.
- **Type safety.**
  In all other macro preprocessors, every value is stored as a string; this often creates
  confusion and bugs. paraphrase, however, has a flexible, dynamic type system; there
  are three built-in types, along with a simple way to define new ones.
- **Robustness.**
  paraphrase generally rejects invalid input, instead of trying to guess at the user's
  intentions. Automatic conversions are kept to a minimum. This allows it to be used
  in situations where safety is important.
- **Flexibility.**
  paraphrase is meant to be just as flexible and powerful as other, untyped macro preprocessors.
  However, it allows users to control and manage this flexibility as much as possible.
  For instance: lexical scope is used by default, but the language provides mechanisms for 
  running code in a dynamically-scoped fashion instead.
- **Minimalism.**
  paraphrase provides a relatively small library of built-in macros, since almost all other
  functionality can be implemented within paraphrase macros. Eventually, a standard library
  (written in paraphrase) will be provided to make programming "from scratch" easier.

# Installation

paraphrase is built in [Rust](https://www.rust-lang.org/en-US/).
It is currently alpha-quality software ("use at your own risk").
To run the basic test suite, install rust and cargo, and run:

```
RUST_BACKTRACE=1 cargo run -- tests/1-simple.pp
```

For further information on the language and its features, see `DOCS.md`
and the other examples in the `tests` folder.

# Credits

paraphrase was completed as an independent study project during Winter Study at Williams College.
I owe a great deal of thanks to Duane Bailey, my advisor there, for helping me to complete this
project.
