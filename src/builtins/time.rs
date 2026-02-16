use std::time::{SystemTime, UNIX_EPOCH};

use super::to_number;

/// Dispatch time built-in functions.
pub fn call(name: &str, args: &[String]) -> String {
    match name {
        "systime" => systime(),
        "strftime" => {
            let fmt = args.first().map(|s| s.as_str()).unwrap_or("%Y-%m-%d %H:%M:%S");
            let ts = args.get(1).map(|s| to_number(s) as i64);
            strftime(fmt, ts)
        }
        "mktime" => {
            let spec = args.first().map(|s| s.as_str()).unwrap_or("");
            mktime(spec)
        }
        "parsedate" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let fmt = args.get(1).map(|s| s.as_str()).unwrap_or("%Y-%m-%d %H:%M:%S");
            parsedate(s, fmt)
        }
        _ => String::new(),
    }
}

/// Return current epoch timestamp as integer seconds.
fn systime() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".to_string(),
    }
}

/// Format a timestamp (or current time) using strftime-style specifiers.
/// Supports: %Y, %m, %d, %H, %M, %S, %s, %%, %A, %B, %a, %b, %Z.
fn strftime(fmt: &str, timestamp: Option<i64>) -> String {
    let ts = match timestamp {
        Some(t) => t,
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };

    let parts = epoch_to_parts(ts);

    let mut result = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            match chars[i] {
                'Y' => result.push_str(&format!("{:04}", parts.year)),
                'm' => result.push_str(&format!("{:02}", parts.month)),
                'd' => result.push_str(&format!("{:02}", parts.day)),
                'H' => result.push_str(&format!("{:02}", parts.hour)),
                'M' => result.push_str(&format!("{:02}", parts.minute)),
                'S' => result.push_str(&format!("{:02}", parts.second)),
                's' => result.push_str(&format!("{}", ts)),
                '%' => result.push('%'),
                'A' => result.push_str(weekday_name(parts.weekday)),
                'a' => result.push_str(&weekday_name(parts.weekday)[..3]),
                'B' => result.push_str(month_name(parts.month)),
                'b' => result.push_str(&month_name(parts.month)[..3]),
                'Z' => result.push_str("UTC"),
                'j' => {
                    let yday = day_of_year(parts.year, parts.month, parts.day);
                    result.push_str(&format!("{:03}", yday));
                }
                'u' => result.push_str(&format!("{}", if parts.weekday == 0 { 7 } else { parts.weekday })),
                'w' => result.push_str(&format!("{}", parts.weekday)),
                'e' => result.push_str(&format!("{:2}", parts.day)),
                'C' => result.push_str(&format!("{:02}", parts.year / 100)),
                'y' => result.push_str(&format!("{:02}", parts.year % 100)),
                'p' => result.push_str(if parts.hour < 12 { "AM" } else { "PM" }),
                'I' => {
                    let h = if parts.hour == 0 { 12 } else if parts.hour > 12 { parts.hour - 12 } else { parts.hour };
                    result.push_str(&format!("{:02}", h));
                }
                'n' => result.push('\n'),
                't' => result.push('\t'),
                _ => {
                    result.push('%');
                    result.push(chars[i]);
                }
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

/// Parse "YYYY MM DD HH MM SS" into epoch seconds (UTC).
fn mktime(spec: &str) -> String {
    let fields: Vec<i64> = spec
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();

    if fields.len() < 6 {
        eprintln!("fk: mktime requires \"YYYY MM DD HH MM SS\"");
        return "-1".to_string();
    }

    let (year, month, day, hour, minute, second) =
        (fields[0], fields[1], fields[2], fields[3], fields[4], fields[5]);

    let epoch = parts_to_epoch(year, month, day, hour, minute, second);
    format!("{}", epoch)
}

// --- date/time arithmetic (UTC, no timezone, no leap seconds) ---

struct DateParts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    weekday: i64, // 0=Sunday
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 => 31, 2 => if is_leap(y) { 29 } else { 28 },
        3 => 31, 4 => 30, 5 => 31, 6 => 30,
        7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
        _ => 30,
    }
}

fn epoch_to_parts(ts: i64) -> DateParts {
    let secs = ts;
    let days = secs.div_euclid(86400);
    let day_secs = secs.rem_euclid(86400);

    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Day of week: Jan 1 1970 was Thursday (4)
    let weekday = (days + 4).rem_euclid(7);

    // Convert days since epoch to year/month/day
    let mut y = 1970i64;
    let mut remaining = days;

    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }

    let mut m = 1i64;
    loop {
        let md = days_in_month(y, m);
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    let d = remaining + 1;

    DateParts { year: y, month: m, day: d, hour, minute, second, weekday }
}

fn parts_to_epoch(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> i64 {
    let mut days: i64 = 0;
    // Count days from 1970 to year
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    // Count days in months
    for m in 1..month {
        days += days_in_month(year, m);
    }
    days += day - 1;
    days * 86400 + hour * 3600 + minute * 60 + second
}

fn weekday_name(d: i64) -> &'static str {
    match d {
        0 => "Sunday", 1 => "Monday", 2 => "Tuesday", 3 => "Wednesday",
        4 => "Thursday", 5 => "Friday", 6 => "Saturday", _ => "Unknown",
    }
}

fn month_name(m: i64) -> &'static str {
    match m {
        1 => "January", 2 => "February", 3 => "March", 4 => "April",
        5 => "May", 6 => "June", 7 => "July", 8 => "August",
        9 => "September", 10 => "October", 11 => "November", 12 => "December",
        _ => "Unknown",
    }
}

fn day_of_year(year: i64, month: i64, day: i64) -> i64 {
    let mut yday = 0;
    for m in 1..month {
        yday += days_in_month(year, m);
    }
    yday + day
}

/// Parse a date string using strftime-style format specifiers.
/// Returns epoch seconds, or -1 on failure.
fn parsedate(s: &str, fmt: &str) -> String {
    let mut year: i64 = 1970;
    let mut month: i64 = 1;
    let mut day: i64 = 1;
    let mut hour: i64 = 0;
    let mut minute: i64 = 0;
    let mut second: i64 = 0;

    let schars: Vec<char> = s.chars().collect();
    let fchars: Vec<char> = fmt.chars().collect();
    let mut si = 0;
    let mut fi = 0;

    while fi < fchars.len() && si < schars.len() {
        if fchars[fi] == '%' && fi + 1 < fchars.len() {
            fi += 1;
            let (val, consumed) = parse_int(&schars, si);
            match fchars[fi] {
                'Y' => { year = val; si += consumed; }
                'm' => { month = val; si += consumed; }
                'd' => { day = val; si += consumed; }
                'H' => { hour = val; si += consumed; }
                'M' => { minute = val; si += consumed; }
                'S' => { second = val; si += consumed; }
                'y' => { year = 2000 + val; si += consumed; }
                _ => { si += consumed; }
            }
        } else if si < schars.len() {
            si += 1;
        }
        fi += 1;
    }

    let epoch = parts_to_epoch(year, month, day, hour, minute, second);
    format!("{}", epoch)
}

fn parse_int(chars: &[char], start: usize) -> (i64, usize) {
    let mut i = start;
    while i < chars.len() && !chars[i].is_ascii_digit() {
        i += 1;
    }
    let begin = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    let val: i64 = chars[begin..i].iter().collect::<String>().parse().unwrap_or(0);
    (val, i - start)
}
