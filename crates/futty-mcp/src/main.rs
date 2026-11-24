fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("Usage: futty-mcp <workspace>")?;
    futty_mcp::run(std::path::Path::new(&root), None)
}
