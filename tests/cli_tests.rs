#[cfg(test)]
pub mod tests {
    use std::process::Command;

    use eyre::Result;

    fn tracing_modes() -> Vec<String> {
        vec!["rpc".to_string(), "revm".to_string()]
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
            let expected_content = ["Real Gas Price:    18253.30 GWEI"];
            for expected in expected_content {
                assert!(
                    output.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{output}"
                );
            }
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
            let expected_content = ["Real Gas Price:    18253.30 GWEI"];
            for expected in expected_content {
                assert!(
                    output.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{output}"
                );
            }
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
        let expected_content = ["setL1BlockValuesIsthmus"];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
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
            let expected_content = [
                "\"tx_hash\": \"0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660\"",
            ];
            for expected in expected_content {
                assert!(
                    output.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{output}"
                );
            }
        }
    }

    #[test]
    fn test_cli_search_from_ens() {
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
            .arg("--ens")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let expected_content = [
            "\"tx_hash\": \"0x71e7d6bb2fc19848cbedbda49f4c49c1ac32bafae0ee0dacd5540b84ca0b7937\"",
            "\"from_ens\": \"jaredfromsubway.eth\"",
            "\"to_ens\": null",
        ];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
    }

    #[test]
    fn test_cli_search_to_ens() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("16733027")
            .arg("--to")
            .arg("jaredfromsubway.eth")
            .arg("--format")
            .arg("json-pretty")
            .arg("--ens")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let expected_content = [
            "\"tx_hash\": \"0x5b5d7168a89bf036b3e2a2b7ce130f5437fd6a60bb4da6f6c719813b3953e01c\"",
            "\"to_ens\": \"jaredfromsubway.eth\"",
            "\"from_ens\": null",
        ];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
    }

    #[test]
    fn test_cli_search_symbols_cache() {
        // Populate symbols cache
        let _ = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("23070298")
            .arg("-p")
            .arg("2")
            .arg("--from")
            .arg("jaredfromsubway.eth")
            .arg("--format")
            .arg("json-pretty")
            .arg("--ens")
            .arg("--erc20-symbols")
            .output()
            .expect("failed to execute CLI");

        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("23070298")
            .arg("-p")
            .arg("2")
            .arg("--from")
            .arg("jaredfromsubway.eth")
            .arg("--format")
            .arg("json-pretty")
            .arg("--ens")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let expected_content = ["\"symbol\": \"WETH\""];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
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
        let expected_content =
            ["\"tx_hash\": \"0x3e8e989819cfc004f7fe58283bf4cc7b39d2ecea5b30e92dc891e06a653371f6\""];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
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
        let expected_content =
            ["\"tx_hash\": \"0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660\""];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
    }

    #[test]
    fn test_cli_format_chain_info() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("chain-info")
            .arg("--chain-id")
            .arg("1")
            .arg("--format")
            .arg("json-pretty")
            .arg("--skip-urls")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let expected_content = ["\"chain_id\": 1", "\"name\": \"Ethereum Mainnet\""];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
    }

    #[test]
    fn test_cli_format_chain_info_error() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("chain-info")
            .arg("--chain-id")
            .arg("0")
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let err = String::from_utf8(cmd.stderr).unwrap();
        assert!(err.contains("\"error\": \"Chain ID 0 not found\""));
    }

    #[test]
    fn test_cli_chains_filter_json() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("chains")
            .arg("--filter")
            .arg("arbitrum")
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let expected_content = ["\"name\": \"Arbitrum One\""];
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}"
            );
        }
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
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let err = String::from_utf8(cmd.stderr).unwrap();
        let expected_content = ["\"error\": \"Invalid block number: 0\""];
        for expected in expected_content {
            assert!(
                err.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{err}"
            );
        }
    }

    #[test]
    fn test_cli_format_search_position_range() {
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
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let json: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap();
        assert_eq!(json.len(), 4);
    }

    #[test]
    fn test_cli_format_tx_create_addr() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("tx")
            .arg("0x7138e07de04d486f99f0117de27026272f33786a5aeeffc0913aef7951dfb1c8")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();

        let expected_content = [
            "\"to\": \"0x7290f841536a3f73835ffad72d27b8c905e1b497\"",
            "\"signature\": \"CREATE()\"",
        ];

        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}",
            );
        }
    }

    #[test]
    fn test_cli_format_search_erc20_transfer() {
        let cmd = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("-b")
            .arg("23305021:23305023")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--format")
            .arg("json-pretty")
            .arg("--sort")
            .arg("erc20Transfer|0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
            .output()
            .expect("failed to execute CLI");

        let expected_content =
            ["\"tx_hash\": \"0xc09b81a9817686083b401b33c8c2df6b09ae4263b15395636bf53e212a0756f4\""];

        let output = String::from_utf8(cmd.stdout).unwrap();
        for expected in expected_content {
            assert!(
                output.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{output}",
            );
        }
    }
}
