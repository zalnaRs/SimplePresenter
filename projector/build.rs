fn main() {
    #[cfg(target_os = "windows")]
    {
	let mut res = winres::WindowsResource::new();
	res.set_icon("resources/logo.ico");
    	res.set("FileVersion", env!("CARGO_PKG_VERSION"));
    	res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
	res.compile().unwrap();
    }
}