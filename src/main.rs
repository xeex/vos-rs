use std::fs::File;
use std::io;
use std::io::BufReader;

mod parser;

fn main() -> io::Result<()> {
    parser::Parser::parse("Aci-L GOD.vow".parse().unwrap());
    Ok(())
}
