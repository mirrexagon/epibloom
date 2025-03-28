pub fn get_current_unix_timestamp() -> i64 {
    time::UtcDateTime::now().unix_timestamp()
}
