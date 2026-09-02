pub mod main {
    use crate::{config::ConfFile, err::StatixErr};
    pub fn main() -> Result<(), StatixErr> {
        println!("{}", ConfFile::dump_default()?);
        Ok(())
    }
}
