pub const SEP: char = '\u{1f}';
pub struct Request {
    pub id: String,
    pub policy: String,
    pub hide: bool,
    pub ui: u32,
    pub timeout: u64,
    pub executable: String,
    pub directory: String,
    pub args: Vec<String>,
}
impl Request {
    pub fn parse(text: &str) -> Result<Self, String> {
        let fields: Vec<_> = text.split(SEP).collect();
        if fields.len() < 9 || fields[0] != "EMB1" {
            return Err("invalid action data".into());
        }
        let policy = fields[2];
        if !matches!(policy, "ignore" | "notify" | "stop") {
            return Err("invalid failure policy".into());
        }
        let hide = match fields[3] {
            "true" => true,
            "false" => false,
            _ => return Err("invalid hide flag".into()),
        };
        let ui = fields[4].parse().map_err(|_| "invalid UI level")?;
        let timeout = fields[5].parse().map_err(|_| "invalid timeout")?;
        let count: usize = fields[8].parse().map_err(|_| "invalid argument count")?;
        if count != fields.len() - 9 || timeout > 86400 {
            return Err("invalid action data size".into());
        }
        Ok(Self {
            id: fields[1].into(),
            policy: policy.into(),
            hide,
            ui,
            timeout,
            executable: fields[6].into(),
            directory: fields[7].into(),
            args: fields[9..].iter().map(|s| s.to_string()).collect(),
        })
    }
}
pub fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        out.push_str(&"\\".repeat(if c == '"' { slashes * 2 + 1 } else { slashes }));
        out.push(c);
        slashes = 0;
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}
