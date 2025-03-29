#[macro_export]
macro_rules! log_debug {
    ($msg:expr, $($args:expr),*) => {
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::time::{SystemTime, UNIX_EPOCH};

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("debug.log")
                .unwrap();

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let log_msg = format!("Timestamp: {} | {} {}", timestamp, $msg, format!($($args),*));
            writeln!(file, "{}", log_msg).unwrap();
        }
    };
}
