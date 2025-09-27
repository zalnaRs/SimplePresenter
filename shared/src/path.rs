use std::path::Path;
use anyhow::anyhow;

pub fn path_to_file_uri(path: &Path) -> anyhow::Result<String> {
    let canonical = path.canonicalize()?;

    #[cfg(windows)]
    {
        let path_str = canonical
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid Unicode in path"))?
            .replace('\\', "/");

        if let Some(stripped) = path_str.strip_prefix("//?/") {
            path_str = stripped.to_string();
        }

        Ok(format!("file:///{}", path_str))
    }

    #[cfg(not(windows))]
    {
        let path_str = canonical
            .to_str()
            .ok_or_else(|| anyhow!("Invalid Unicode in path"))?;

        Ok(format!("file://{}", path_str))
    }
}