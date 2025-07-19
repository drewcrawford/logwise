#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
fn console_log(msg: &str) {
    web_sys::console::log_1(&format!("[WASM-DEBUG] {}", msg).into());
}

#[cfg(not(target_arch = "wasm32"))]
fn console_log(msg: &str) {
    println!("[DEBUG] {}", msg);
}

#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

#[cfg(not(target_arch = "wasm32"))]
use std::thread;

#[test_executors::async_test]
async fn test_log() {
    let (c,r) = r#continue::continuation();
    let _spawn_result = thread::spawn(move || {
        // This might work better - write to main thread's console
        console_log(&"Thread started execution");
        console_log(&"Hello, world INFO from thread in CONSOLE");
        console_log(&"[WASM-DEBUG] Thread about to send continuation");
        c.send(());
        console_log(&"[WASM-DEBUG] Thread sent continuation");
    });
    console_log("Thread spawned successfully");
    console_log("Waiting for continuation");
    r.await;
    console_log("Continuation received");
    //panic!("This intentional panic is to demonstrate logging in tests.");
}