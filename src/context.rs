use std::cell::Cell;
use std::collections::HashMap;
use crate::privacy::Loggable;


/**
Provides a set of info that can be used by multiple logs.
*/
pub struct Context {
    parent: Option<Box<Context>>,
    values: HashMap<&'static str, Box<dyn Loggable>>
}

thread_local! {
    static CONTEXT: Cell<Option<Context>> = const{Cell::new(None)};
}

impl Context {
    /**
    Determines whether a context is currently set.
    */
    pub fn has_current() -> bool {
        CONTEXT.with(|c| {
            let context = c.take();
            let result = context.is_some();
            c.set(context);
            result
        })
    }
    /**
    Returns a raw pointer to the thread-local context.

    When using this pointer, you must ensure that the context is only accessed from the local thread,
    and is not mutated by the local thread while the pointer is in use.
    */
    pub fn current() -> *const Option<Context> {
        CONTEXT.with(|c| {
            c.as_ptr()
        })
    }
    /**
    Creates a new orphaned context
    */
    pub fn new_orphan() -> Context {
        Context { parent: None,
            values: HashMap::new() }
    }

    /**
    Sets a blank context
    */
    pub fn reset() {
        CONTEXT.with(|c| c.set(Some(Context::new_orphan())));
    }

    /**
    Sets the current context to this one.
    */
    pub fn set_current(self) {
        CONTEXT.set(Some(self));
    }

    /**
    Pushes the current context onto the stack and sets this context as the current one.
    */
    pub fn push(mut self) {
        let current = CONTEXT.with(|c| c.take());
        match current {
            Some(c) => {
                self.parent = Some(Box::new(c));
            }
            None => {}
        }
        CONTEXT.with(|c| c.set(Some(self)));
    }

    pub fn set<L: Loggable + 'static>(&mut self, key: &'static str, value: L) {
        self.values.insert(key, Box::new(value));
    }





}