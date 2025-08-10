#[cfg(test)]
pub mod tests {
    use std::process::Command;

    use eyre::Result;

    fn tracing_modes() -> Vec<String> {
        vec![
            "rpc".to_string(),
            #[cfg(feature = "revm-integration")]
            "revm".to_string(),
        ]
    }

    #[test]
    fn test_cli_search() -> Result<()> {
        for tracing_mode in tracing_modes() {
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
                .arg(tracing_mode)
                .output()
                .expect("failed to execute CLI");

            let output = String::from_utf8(cmd.stdout).unwrap();
            println!("output: {output}");
            assert!(output.contains("Real Gas Price:    18253.30 GWEI"));
        }

        Ok(())
    }

    #[test]
    fn test_cli_tx() -> Result<()> {
        for tracing_mode in tracing_modes() {
            let cmd = Command::new("cargo")
                .arg("run")
                .arg("--bin")
                .arg("mevlog")
                .arg("tx")
                .arg("0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660")
                .arg("--rpc-url")
                .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
                .arg("--trace")
                .arg(tracing_mode)
                .output()
                .expect("failed to execute CLI");

            let output = String::from_utf8(cmd.stdout).unwrap();
            println!("output: {output}");
            assert!(output.contains("Real Gas Price:    18253.30 GWEI"));
        }

        Ok(())
    }

    #[test]
    fn test_sig_overwrite() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("33410345")
            .arg("-p")
            .arg("0")
            .arg("--rpc-url")
            .arg(std::env::var("BASE_RPC_URL").expect("BASE_RPC_URL must be set"))
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains("setL1BlockValuesIsthmus"));
    }

    #[test]
    fn test_cli_search_touching() {
        for tracing_mode in tracing_modes() {
            let cmd = Command::new("cargo")
                .arg("run")
                .arg("--bin")
                .arg("mevlog")
                .arg("search")
                .arg("-b")
                .arg("22045570")
                .arg("-p")
                .arg("0:3")
                .arg("--rpc-url")
                .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
                .arg("--trace")
                .arg(tracing_mode)
                .arg("--format")
                .arg("json-pretty")
                .arg("--touching")
                .arg("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
                .output()
                .expect("failed to execute CLI");
            let output = String::from_utf8(cmd.stdout).unwrap();
            println!("output: {output}");
            assert!(output.contains("\"tx_hash\": \"0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660\""));
        }
    }

    #[test]
    fn test_cli_search_ens() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("23070298")
            .arg("-p")
            .arg("0:8")
            .arg("--from")
            .arg("jaredfromsubway.eth")
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains(
            "\"tx_hash\": \"0x71e7d6bb2fc19848cbedbda49f4c49c1ac32bafae0ee0dacd5540b84ca0b7937\""
        ));
    }

    #[test]
    fn test_cli_search_sort_limit() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("22045570")
            .arg("-p")
            .arg("0:50")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--format")
            .arg("json-pretty")
            .arg("--sort")
            .arg("gas-price")
            .arg("--limit")
            .arg("1")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains(
            "\"tx_hash\": \"0x3e8e989819cfc004f7fe58283bf4cc7b39d2ecea5b30e92dc891e06a653371f6\""
        ));
    }

    #[test]
    fn test_cli_search_sort_limit_asc() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("22045570")
            .arg("-p")
            .arg("0:50")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--format")
            .arg("json-pretty")
            .arg("--sort")
            .arg("gas-price")
            .arg("--sort-dir")
            .arg("asc")
            .arg("--limit")
            .arg("1")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        println!("output: {output}");
        assert!(output.contains(
            "\"tx_hash\": \"0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660\""
        ));
    }

    #[test]
    fn test_cli_format_chain_info() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("chain-info")
            .arg("--chain-id")
            .arg("0")
            .arg("--format")
            .arg("json")
            .output()
            .expect("failed to execute CLI");

        let err = String::from_utf8(cmd.stderr).unwrap();
        println!("err: {err}");
        assert!(err.contains("\"error\":\"Chain ID 0 not found\""));
    }

    #[test]
    fn test_cli_format_search() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("0")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--format")
            .arg("json")
            .output()
            .expect("failed to execute CLI");

        let err = String::from_utf8(cmd.stderr).unwrap();
        println!("err: {err}");
        assert!(err.contains("\"error\":\"Invalid block number: 0\""));
    }
}
