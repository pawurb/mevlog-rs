#[cfg(test)]
pub mod tests {
    use std::process::Command;

    use eyre::Result;

    #[test]
    fn test_cli_search() -> Result<()> {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("22045570")
            .arg("-p")
            .arg("0")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--trace")
            .arg("rpc")
            .output()
            .expect("failed to execute CLI");

        if !cmd.stderr.is_empty() {
            println!(
                "stderr: {:?}",
                String::from_utf8(cmd.stderr.clone()).unwrap()
            );
        }

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains("Real Gas Price:    18253.30 GWEI"));

        Ok(())
    }

    #[test]
    fn test_cli_tx() -> Result<()> {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("tx")
            .arg("0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--trace")
            .arg("rpc")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains("Real Gas Price:    18253.30 GWEI"));

        Ok(())
    }
}
