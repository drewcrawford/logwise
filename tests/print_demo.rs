wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
fn console_log(msg: &str) {
    web_sys::console::log_1(&format!("[WASM-DEBUG] {}", msg).into());
}

#[cfg(not(target_arch = "wasm32"))]
fn console_log(msg: &str) {
    println!("[DEBUG] {}", msg);
}

#[wasm_bindgen_test::wasm_bindgen_test]
fn test_log() {
    console_log("Hello, world INFO in CONSOLE");
    console_log("About to spawn thread");
    let (c,r) = r#continue::continuation();
    let _spawn_result = wasm_thread::spawn(move || {
        // This might work better - write to main thread's console
        web_sys::console::log_1(&"[WASM-DEBUG] Thread started execution".into());
        web_sys::console::log_1(&"[WASM-DEBUG] Hello, world INFO from thread in CONSOLE".into());
        web_sys::console::log_1(&"[WASM-DEBUG] Thread about to send continuation".into());
        c.send(());
        web_sys::console::log_1(&"[WASM-DEBUG] Thread sent continuation".into());
    });
    console_log("Thread spawned successfully");
    console_log("Waiting for continuation");
    r.await;
    console_log("Continuation received");
    panic!("This intentional panic is to demonstrate logging in tests.");
}