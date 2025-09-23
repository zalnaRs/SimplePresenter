use serde::Serialize;

pub mod client;
#[derive(Debug, Clone, Serialize)]
pub enum Skip {
    VideoEnd,
    None,
    Time(u32),
}

impl Skip {
    pub fn to_string(&self) -> String {
        match self {
            Skip::VideoEnd => "VideoEnd".to_string(),
            Skip::None => "None".to_string(),
            Skip::Time(secs) => format!("Time({})", secs),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "VideoEnd" => Some(Skip::VideoEnd),
            "None" => Some(Skip::None),
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
    VideoEnded
}