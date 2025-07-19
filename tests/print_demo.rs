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

use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use logwise::InMemoryLogger;

#[test_executors::async_test]
async fn test_log() {
    let logger = Arc::new(InMemoryLogger::new());
    let drain_logger = logger.clone();
    logwise::set_global_logger(logger);
    let (c,r) = r#continue::continuation();
    let _spawn_result = thread::spawn(move || {
        // This might work better - write to main thread's console
        logwise::info_sync!("Thread started execution");
        logwise::info_sync!("Hello, world INFO from thread in CONSOLE");
        logwise::info_sync!("[WASM-DEBUG] Thread about to send continuation");
        c.send(());
        logwise::info_sync!("[WASM-DEBUG] Thread sent continuation");
    });
    logwise::info_sync!("Thread spawned successfully");
    logwise::info_sync!("Waiting for continuation");
    r.await;
    logwise::info_sync!("Continuation received");
    let drain_logs = drain_logger.drain_logs();
    console_log(&drain_logs);
    //panic!("This intentional panic is to demonstrate logging in tests.");
}