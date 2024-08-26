pub struct _WarnLogger;
impl _WarnLogger {
    pub fn write_literal(&self, s: &str) {
        println!("{}",s);
    }
    pub fn write_val(&self, s: u8) {
        println!("{}",s);
    }
}

pub static _WARN_LOGGER: _WarnLogger = _WarnLogger;

/**
Logs a message at warning level.

```
use dlog::warn;
warn!("Hello {world}",world=23);
```
*/

#[macro_export]
macro_rules! warn {
    //pass to lformat!
    ($($arg:tt)*) => {
        let logger = &$crate::hidden::_WARN_LOGGER;
        dlog_proc::lformat!(logger,$($arg)*);
    };
}