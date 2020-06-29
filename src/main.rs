mod parser;

fn main() -> std::io::Result<()> {
    parser::Parser::parse("Aci-L GOD.vow".parse().unwrap())?;
    Ok(())
}
