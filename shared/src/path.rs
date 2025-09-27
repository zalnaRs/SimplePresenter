use std::path::Path;
use anyhow::anyhow;

pub fn path_to_file_uri(path: &Path) -> anyhow::Result<String> {
    let canonical = path.canonicalize()?;

    #[cfg(windows)]
    {
        // Windows needs slashes converted and drive letter handled
        let path_str = canonical
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid Unicode in path"))?
            .replace('\\', "/");

        // Ensure it starts with a slash for file:///C:/...
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