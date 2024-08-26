use proc_macro::{TokenStream, TokenTree, Literal};
use std::collections::{HashMap, VecDeque};


fn build_kvs(input: &mut VecDeque<TokenTree>) -> Result<HashMap<String,Literal>,TokenStream> {
    let mut kvs = HashMap::new();
    loop {
        //first extract the comma.
        match input.pop_front() {
            Some(TokenTree::Punct(p)) => {
                if p.as_char() == ',' {
                    continue;
                }
                return Err(r#"compile_error!("Expected ','")"#.parse().unwrap());
            }
            Some(TokenTree::Ident(i)) => {
                let key = i.to_string();
                let next = input.pop_front();
                match next {
                    Some(TokenTree::Punct(p)) => {
                        if p.as_char() == '=' {
                            //expect literal
                            match input.pop_front() {
                                Some(TokenTree::Literal(l)) => {
                                    kvs.insert(key,l);
                                    continue;
                                }
                                _ => {
                                    return Err(r#"compile_error!("Expected literal")"#.parse().unwrap());
                                }
                            }
                        }
                        return Err(r#"compile_error!("Expected '='")"#.parse().unwrap());
                    }
                    _ => {
                        return Err(r#"compile_error!("Expected '='")"#.parse().unwrap());
                    }
                }

            }
            None => {
                return Ok(kvs);
            }
            Some(thing) => {
                let msg = format!("compile_error!(\"Unexpected token {}\")",thing.to_string().replace("\"","\\\""));
                return Err(msg.parse().unwrap());
            }
        }
    }


}
/**
Replaces a format string with a sequence of log calls.

```
# struct Logger;
# impl Logger {
#   fn write_literal(&self, s: &str) {}
#   fn write_val(&self, s: u8) {}
# }
# let logger = Logger;
use dlog_proc::lformat;
lformat!(logger,"Hello, {world}!", world=23);
```

This becomes:
```ignore
logger.write_literal("Hello, ");
logger.write_val(23);
logger.write_literal("!");
logger.add_context("world",23);
```

# Additional test coverage

```compile_fail
use dlog_proc::lformat;
lformat!(23);
```
*/
#[proc_macro]
pub fn lformat(input: TokenStream) -> TokenStream {
    let mut collect: VecDeque<_> = input.into_iter().collect();

    //get logger ident
    let logger_ident = match collect.pop_front() {
        Some(TokenTree::Ident(i)) => i,
        _ => {
            return r#"compile_error!("lformat!() must be called with a logger ident")"#.parse().unwrap();
        }
    };
    //eat comma
    match collect.pop_front() {
        Some(TokenTree::Punct(p)) => {
            if p.as_char() != ',' {
                return r#"compile_error!("Expected ','")"#.parse().unwrap();
            }
        }
        _ => {
            return r#"compile_error!("Expected ','")"#.parse().unwrap();
        }
    }

    let some_input = match collect.remove(0) {
        Some(i) => i,
        None => {
            return r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap();
        }
    };
    let format_string = match some_input {
        TokenTree::Literal(l) => {
            let out = l.to_string();
            if !out.starts_with('"') || !out.ends_with('"') {
                return r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap();
            }
            out[1..out.len()-1].to_string()

        }
        _ => {
            return r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap();
        }
    };

    //parse kv section
    let k = match build_kvs(&mut collect) {
        Ok(kvs) => kvs,
        Err(e) => {
            return e;
        }
    };
    //parse format string
    //holds the part of the string literal until the next {
    let mut source = String::new();
    enum Mode {
        Literal(String),
        Key(String),
    }
    let mut mode = Mode::Literal(String::new());

    for(c,char) in format_string.chars().enumerate() {
        match mode {
            Mode::Literal(mut literal) => {
                if char == '{' {
                    //peek to see if we're escaping
                    if format_string.chars().nth(c+1) == Some('{') {
                        literal.push(char);
                        mode = Mode::Literal(literal);
                    }
                    else if !literal.is_empty() {
                        //reference logger ident
                        source.push_str(&logger_ident.to_string());
                        source.push_str(".write_literal(\"");
                        source.push_str(&literal);
                        source.push_str("\");\n");
                        mode = Mode::Key(String::new());
                    }
                    else {
                        mode = Mode::Key(String::new());
                    }
                }
                else {
                    literal.push(char);
                    mode = Mode::Literal(literal);
                }

            }
            Mode::Key(mut key) => {
                if char == '}' {
                    //write out the key
                    source.push_str(&logger_ident.to_string());
                    source.push_str(".write_val(");
                    let value = match k.get(&key) {
                        Some(l) => l.to_string(),
                        None => {
                            return format!(r#"compile_error!("Key {} not found")"#, key).parse().unwrap();
                        }
                    };
                    source.push_str(&value);
                    source.push_str(");\n");
                    mode = Mode::Literal(String::new());
                } else {
                    key.push(char);
                    mode = Mode::Key(key);
                }
            }

        }
    }
    //check end situation
    match mode {
        Mode::Literal(l) => {
            if !l.is_empty() {
                source.push_str(&logger_ident.to_string());
                source.push_str(".write_literal(\"");
                source.push_str(&l);
                source.push_str("\");\n");
            }
        }
        Mode::Key(_) => {
            return r#"compile_error!("Expected '}'")"#.parse().unwrap();
        }
    }




    source.parse().unwrap()
}

#[proc_macro_attribute]
pub fn perfwarn(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let literal = match attr.into_iter().next() {
        Some(TokenTree::Literal(lit)) => lit,
        _ => {
            return r#"compile_error!("Expected a string literal argument")"#.parse().unwrap();
        }
    };



    //generate the output

    let o = format!(
        "
        let interval = ::dlog::perfwarn_begin!({LITERAL});
        {ITEM}
        drop(interval);
    ", LITERAL = literal, ITEM = item);

    // Return the generated code
    o.parse().unwrap()
}