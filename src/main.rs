mod parser;

fn main() -> std::io::Result<()> {
    parser::Parser::parse("path_to_vos".parse().unwrap())?;
    Ok(())
}
