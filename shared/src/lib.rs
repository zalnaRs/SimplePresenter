use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum Skip {
    VideoEnd,
    Input,
    Time(u32),
}

impl Skip {
    pub fn to_string(&self) -> String {
        match self {
            Skip::VideoEnd => "VideoEnd".to_string(),
            Skip::Input => "Input".to_string(),
            Skip::Time(secs) => format!("Time({})", secs),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "VideoEnd" => Some(Skip::VideoEnd),
            "Input" => Some(Skip::Input),
            _ if s.starts_with("Time(") && s.ends_with(")") => {
                let inner = &s[5..s.len() - 1];
                inner.parse::<u32>().ok().map(Skip::Time)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    path: String,
    skip: Skip,
}


#[derive(Debug)]
pub enum ProjectorCommand {
    Start { path: String, skip: String },
}