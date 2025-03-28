use time::UtcDateTime;

pub fn get_current_unix_timestamp() -> i64 {
    UtcDateTime::now().unix_timestamp()
}
