// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for the context module.

use super::context_impl::Context;
use super::task::{Task, TaskID};
use crate::Level;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_new_context() {
    Context::reset("test_new_context".to_string());
    let port_context = Context::current();
    let next_context = Context::from_parent(port_context);
    let next_context_id = next_context.context_id();
    next_context.set_current();

    Context::pop(next_context_id);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_context_equality() {
    Context::reset("test_context_equality".to_string());
    let context1 = Context::current();
    let context2 = context1.clone();
    let context3 = Context::new_task(None, "different_task".to_string(), Level::Info, true);

    // Same Arc pointer should be equal
    assert_eq!(context1, context2);

    // Different Arc pointers should not be equal
    assert_ne!(context1, context3);
    assert_ne!(context2, context3);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[allow(clippy::mutable_key_type)] // Context hash is based on Arc pointer, not interior state
fn test_context_hash() {
    use std::collections::HashMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    Context::reset("test_context_hash".to_string());
    let context1 = Context::current();
    let context2 = context1.clone();
    let context3 = Context::new_task(None, "different_task".to_string(), Level::Info, true);

    // Same Arc pointer should have same hash
    let mut hasher1 = DefaultHasher::new();
    let mut hasher2 = DefaultHasher::new();
    context1.hash(&mut hasher1);
    context2.hash(&mut hasher2);
    assert_eq!(hasher1.finish(), hasher2.finish());

    // Different Arc pointers should have different hashes (highly likely)
    let mut hasher3 = DefaultHasher::new();
    context3.hash(&mut hasher3);
    assert_ne!(hasher1.finish(), hasher3.finish());

    // Test that Context can be used as HashMap key
    let mut map = HashMap::new();
    map.insert(context1.clone(), "value1");
    map.insert(context3.clone(), "value3");

    assert_eq!(map.get(&context1), Some(&"value1"));
    assert_eq!(map.get(&context2), Some(&"value1")); // same as context1
    assert_eq!(map.get(&context3), Some(&"value3"));
    assert_eq!(map.len(), 2); // only 2 unique contexts
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_context_display() {
    Context::reset("root_task".to_string());
    let root_context = Context::current();

    // Root context should have no indentation (nesting level 0)
    let root_display = format!("{}", root_context);
    assert!(root_display.starts_with(&format!("{} (root_task)", root_context.task_id())));
    assert!(!root_display.starts_with("  ")); // no indentation

    // Create a child context
    let child_context = Context::from_parent(root_context.clone());
    child_context.clone().set_current();
    let child_display = format!("{}", child_context);

    // Child should have 1 level of indentation
    assert!(child_display.starts_with("  ")); // 2 spaces for nesting level 1
    assert!(child_display.contains(&format!("{} (root_task)", root_context.task_id())));

    // Create a new task context as child
    let task_context = Context::new_task(
        Some(child_context.clone()),
        "child_task".to_string(),
        Level::Info,
        true,
    );
    task_context.clone().set_current();
    let task_display = format!("{}", task_context);

    // Task context should have 2 levels of indentation
    assert!(task_display.starts_with("    ")); // 4 spaces for nesting level 2
    assert!(task_display.contains(&format!("{} (child_task)", task_context.task_id())));

    // Create grandchild
    let grandchild_context = Context::from_parent(task_context.clone());
    grandchild_context.clone().set_current();
    let grandchild_display = format!("{}", grandchild_context);

    // Grandchild should have 3 levels of indentation
    assert!(grandchild_display.starts_with("      ")); // 6 spaces for nesting level 3
    assert!(grandchild_display.contains(&format!("{} (child_task)", task_context.task_id())));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn test_context_as_ref_task() {
    Context::reset("test_as_ref".to_string());
    let context = Context::current();

    // Test that AsRef<Task> works
    let task_ref: &Task = context.as_ref();
    assert_eq!(task_ref.task_id, context.task_id());
    assert_eq!(task_ref.label, "test_as_ref");

    // Test that we can use Context where &Task is expected
    fn takes_task_ref(task: &Task) -> TaskID {
        task.task_id
    }

    // Test explicit AsRef usage
    let id1 = takes_task_ref(context.as_ref());
    assert_eq!(id1, context.task_id());

    // Test with generic function that accepts AsRef<Task>
    fn takes_as_ref_task<T: AsRef<Task>>(item: T) -> TaskID {
        item.as_ref().task_id
    }

    let id2 = takes_as_ref_task(&context);
    let id3 = takes_as_ref_task(context.clone());
    assert_eq!(id1, id2);
    assert_eq!(id2, id3);

    // Test with different context types
    let child_context = Context::from_parent(context.clone());
    let child_task_ref: &Task = child_context.as_ref();

    // Child should have same task as parent (since from_parent preserves task)
    assert_eq!(child_task_ref.task_id, context.task_id());
    assert_eq!(child_task_ref.label, "test_as_ref");

    let new_task_context = Context::new_task(
        Some(context.clone()),
        "new_task".to_string(),
        Level::Info,
        true,
    );
    let new_task_ref: &Task = new_task_context.as_ref();

    // New task should have different ID and label
    assert_ne!(new_task_ref.task_id, context.task_id());
    assert_eq!(new_task_ref.label, "new_task");
}
