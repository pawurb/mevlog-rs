#[cfg(test)]
pub mod tests {
    use std::process::Command;

    use eyre::Result;

    #[test]
    fn test_cli_search() -> Result<()> {
        let cmd = Command::new("./target/debug/mevlog")
            .arg("search")
            .arg("-b")
            .arg("22045570")
            .arg("-p")
            .arg("0")
            .arg("--rpc-url")
            .arg("https://eth.merkle.io")
            .arg("--trace")
            .arg("revm")
            .output()
            .expect("failed to execute CLI");

        if !cmd.stderr.is_empty() {
            println!(
                "stderr: {:?}",
                String::from_utf8(cmd.stderr.clone()).unwrap()
            );
        }

        assert!(cmd.stderr.is_empty());
        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {}", output);
        assert!(output.contains("Real Gas Price:    18253.30 GWEI"));

        Ok(())
    }

    #[test]
    fn test_cli_tx() -> Result<()> {
        let cmd = Command::new("./target/debug/mevlog")
            .arg("tx")
            .arg("0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660")
            .arg("--rpc-url")
            .arg("https://eth.merkle.io")
            .arg("--trace")
            .arg("revm")
            .output()
            .expect("failed to execute CLI");

        if !cmd.stderr.is_empty() {
            println!(
                "stderr: {:?}",
                String::from_utf8(cmd.stderr.clone()).unwrap()
            );
        }

        assert!(cmd.stderr.is_empty());
        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {}", output);
        assert!(output.contains("Real Gas Price:    18253.30 GWEI"));

        Ok(())
    }
}
