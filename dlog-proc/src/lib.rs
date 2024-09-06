use proc_macro::{TokenStream, TokenTree};
use std::collections::{HashMap, VecDeque};


fn build_kvs(input: &mut VecDeque<TokenTree>) -> Result<HashMap<String,String>,TokenStream> {
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
                            let next = input.pop_front();
                            match next {
                                Some(TokenTree::Literal(l)) => {
                                    kvs.insert(key,l.to_string());
                                    continue;
                                }
                                Some(TokenTree::Ident(i)) => {
                                    kvs.insert(key, i.to_string());
                                    continue;
                                }
                                _ => {
                                    let next_desc = format!("{:?}",next).replace("\"","\\\"");
                                    let r = format!(r#"compile_error!("Expected literal near {}")"#, next_desc);
                                    return Err(r.parse().unwrap());
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

struct LFormatResult {
    output: TokenStream,
    name: String,
}

fn lformat_impl(collect: &mut VecDeque<TokenTree>,logger: String) -> LFormatResult {
    let some_input = match collect.remove(0) {
        Some(i) => i,
        None => {
            return LFormatResult{output: r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap(), name: "".to_string()}
        }
    };
    let format_string = match some_input {
        TokenTree::Literal(l) => {
            let out = l.to_string();
            if !out.starts_with('"') || !out.ends_with('"') {
                return LFormatResult{output: r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap(), name: "".to_string()};
            }
            out[1..out.len()-1].to_string()

        }
        _ => {
            return LFormatResult{output: r#"compile_error!("lformat!() must be called with a string literal")"#.parse().unwrap(), name: "".to_string()};
        }
    };

    //parse kv section
    let k = match build_kvs(collect) {
        Ok(kvs) => kvs,
        Err(e) => {
            return LFormatResult{output: e, name: "".to_string()};
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
                        source.push_str(&logger);
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
                    source.push_str(&logger);
                    source.push_str(".write_val(");
                    let value = match k.get(&key) {
                        Some(l) => l.to_string(),
                        None => {
                            return LFormatResult{output: format!(r#"compile_error!("Key {} not found")"#, key).parse().unwrap(), name: "".to_string()};
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
                source.push_str(&logger.to_string());
                source.push_str(".write_literal(\"");
                source.push_str(&l);
                source.push_str("\");\n");
            }
        }
        Mode::Key(_) => {
            return LFormatResult{output: r#"compile_error!("Expected '}'")"#.parse().unwrap(), name: "".to_string()};
        }
    }
    return LFormatResult{output: source.parse().unwrap(), name: format_string};
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

    let o = lformat_impl(&mut collect,logger_ident.to_string());
    o.output

}



/**
Logs a message at debug_internal level
*/
#[proc_macro] pub fn debuginternal_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(r#"
        #[cfg(debug_assertions)]
        if module_path!().starts_with(env!("CARGO_PKG_NAME")) {{
                let mut record = dlog::hidden::debuginternal_pre(file!(),line!(),column!());
                let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                dlog::hidden::debuginternal_sync_post(record);
       }}
    "#, LFORMAT_EXPAND=lformat_result.output);

    src.parse().unwrap()

}

#[proc_macro] pub fn debuginternal_async(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(r#"
        #[cfg(debug_assertions)] {{
            if module_path!().starts_with(env!("CARGO_PKG_NAME")) {{
               let mut record = dlog::hidden::debuginternal_pre(file!(),line!(),column!());
                let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                dlog::hidden::debuginternal_async_post(record).await;
            }}
        }}
    "#, LFORMAT_EXPAND=lformat_result.output);

    src.parse().unwrap()

}

/**
Logs a message at info level.
 */

#[proc_macro] pub fn info_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(r#"
        #[cfg(debug_assertions)]
        {{
            let mut record = dlog::hidden::info_sync_pre(file!(),line!(),column!());

            let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            dlog::hidden::info_sync_post(record);
        }}
    "#, LFORMAT_EXPAND=lformat_result.output);

    src.parse().unwrap()

}

/**
Logs a message at warning leve.
*/
#[proc_macro] pub fn warn_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(r#"
        {{
            let mut record = dlog::hidden::warn_sync_pre(file!(),line!(),column!());

            let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            dlog::hidden::warn_sync_post(record);
        }}
    "#, LFORMAT_EXPAND=lformat_result.output);

    src.parse().unwrap()

}

/**
Logs a performance warning interval.


*/
#[proc_macro] pub fn perfwarn_begin(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(r#"
        {{
            let mut record = dlog::hidden::perfwarn_begin_pre(file!(),line!(),column!());
            let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);
            {LFORMAT_EXPAND}
            dlog::hidden::perfwarn_begin_post(record,"{NAME}")
        }}
    "#, LFORMAT_EXPAND=lformat_result.output,NAME=lformat_result.name);
    src.parse().unwrap()

}

/**
Logs a performance warning interval.


*/
#[proc_macro] pub fn perfwarn(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let last_token = input.pop_back().expect("Expected block");
    let lformat_expand = lformat_impl(&mut input, "formatter".to_string());

    let group = match last_token {
        TokenTree::Group(g) => {
           g
        }
        _ => {
            return r#"compile_error!("Expected block")"#.parse().unwrap()
        }
    };
    if group.delimiter() != proc_macro::Delimiter::Brace {
        return r#"compile_error!("Expected block")"#.parse().unwrap()
    }

    let src = format!(r#"
        {{
            let mut record = dlog::hidden::perfwarn_begin_pre(file!(),line!(),column!());
            let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);
            {LFORMAT_EXPAND}
            let interval = dlog::hidden::perfwarn_begin_post(record,"{NAME}");
            let result = {BLOCK};
            drop(interval);
            result
        }}
    "#, LFORMAT_EXPAND=lformat_expand.output,BLOCK=group.to_string(),NAME=lformat_expand.name);
    src.parse().unwrap()
}