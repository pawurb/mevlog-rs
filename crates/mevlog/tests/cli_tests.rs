#[cfg(test)]
pub mod tests {
    use std::process::Command;

    use eyre::Result;

    fn tracing_modes() -> Vec<String> {
        vec!["rpc".to_string(), "revm".to_string()]
    }

    // cargo run --bin mevlog search -b 22045570 -p 0 --rpc-url $ETH_RPC_URL --evm-trace rpc --format json-pretty
    #[test]
    fn test_cli_search() -> Result<()> {
        for tracing_mode in tracing_modes() {
            let cmd = Command::new("cargo")
                .env("RUST_LOG", "off")
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
                .arg("--evm-trace")
                .arg(tracing_mode)
                .arg("--format")
                .arg("json-pretty")
                .output()
                .expect("failed to execute CLI");

            let output = String::from_utf8(cmd.stdout).unwrap();
            let expected_content = ["\"full_tx_cost\""];
            for expected in expected_content {
                assert!(
                    output.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{output}"
                );
            }
        }

        Ok(())
    }

    // cargo run --bin mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --rpc-url $ETH_RPC_URL --evm-trace rpc --format json-pretty
    #[test]
    fn test_cli_tx() -> Result<()> {
        for tracing_mode in tracing_modes() {
            let cmd = Command::new("cargo")
                .env("RUST_LOG", "off")
                .arg("run")
                .arg("--bin")
                .arg("mevlog")
                .arg("tx")
                .arg("0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660")
                .arg("--rpc-url")
                .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
                .arg("--evm-trace")
                .arg(tracing_mode)
                .arg("--format")
                .arg("json-pretty")
                .output()
                .expect("failed to execute CLI");

            let output = String::from_utf8(cmd.stdout).unwrap();
            let expected_content = ["\"full_tx_cost\""];
            for expected in expected_content {
                assert!(
                    output.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{output}"
                );
            }
        }

        Ok(())
    }

    // cargo run --bin mevlog search -b 33410345 -p 0 --rpc-url $BASE_RPC_URL --format json-pretty
    #[test]
    fn test_sig_overwrite() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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
            .arg("--format")
            .arg("json-pretty")
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

    // cargo run --bin mevlog search -b 22045570 -p 0:3 --rpc-url $ETH_RPC_URL --evm-trace rpc --format json-pretty --touching 0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640
    #[test]
    fn test_cli_search_touching() {
        for tracing_mode in tracing_modes() {
            let cmd = Command::new("cargo")
                .env("RUST_LOG", "off")
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
                .arg("--evm-trace")
                .arg(tracing_mode)
                .arg("--format")
                .arg("json-pretty")
                .arg("--touching")
                .arg("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
                .arg("--logs")
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
            assert!(
                output.contains("\"logs\":"),
                "Expected logs to be present when --logs is set.\n\nGot:\n{output}"
            );
        }
    }

    // cargo run --bin mevlog search -b 23070298 -p 0:8 --from jaredfromsubway.eth --format json-pretty --ens --rpc-url $ETH_RPC_URL
    #[test]
    fn test_cli_search_from_ens() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
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

    // cargo run --bin mevlog search -b 16733027 --to jaredfromsubway.eth --format json-pretty --ens --rpc-url $ETH_RPC_URL
    #[test]
    fn test_cli_search_to_ens() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
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

    // cargo run --bin mevlog search -b 23070298 -p 2 --from jaredfromsubway.eth --format json-pretty --ens --rpc-url $ETH_RPC_URL
    #[test]
    fn test_cli_search_symbols_cache() {
        // Populate symbols cache
        let _ = Command::new("cargo")
            .env("RUST_LOG", "off")
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
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .output()
            .expect("failed to execute CLI");

        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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
            .arg("--logs")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
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

    // cargo run --bin mevlog search -b 22045570 -p 0:50 --rpc-url $ETH_RPC_URL --format json-pretty --sort gas-price --limit 1
    #[test]
    fn test_cli_search_sort_limit() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

    // cargo run --bin mevlog search -b 22045570 -p 0:50 --rpc-url $ETH_RPC_URL --format json-pretty --sort gas-price --sort-dir asc --limit 1
    #[test]
    fn test_cli_search_sort_limit_asc() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

    // cargo run --bin mevlog chain-info --chain-id 1 --format json-pretty --skip-urls
    #[test]
    fn test_cli_format_chain_info() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("chain-info")
            .arg("--chain-id")
            .arg("1")
            .arg("--format")
            .arg("json-pretty")
            .arg("--skip-rpcs")
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

    // cargo run --bin mevlog chain-info --chain-id 0 --format json-pretty
    #[test]
    fn test_cli_format_chain_info_error() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

    // cargo run --bin mevlog chains --filter arbitrum --format json-pretty
    #[test]
    fn test_cli_chains_filter_json() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

    // cargo run --bin mevlog search -b 0 --rpc-url $ETH_RPC_URL --format json-pretty
    #[test]
    fn test_cli_format_search() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

    // cargo run --bin mevlog search -b 22045570 -p 0:3 --rpc-url $ETH_RPC_URL --format json-pretty
    #[test]
    fn test_cli_format_search_position_range() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

        let err = String::from_utf8(cmd.stderr).unwrap();
        assert!(cmd.status.success(), "CLI failed:\n{err}");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        let txs = json["result"]
            .as_array()
            .expect("result should be an array");
        assert_eq!(txs.len(), 4);
    }

    // cargo run --bin mevlog tx 0x7138e07de04d486f99f0117de27026272f33786a5aeeffc0913aef7951dfb1c8 --rpc-url $ETH_RPC_URL --format json-pretty
    #[test]
    fn test_cli_format_tx_create_addr() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        let txs = json["result"]
            .as_array()
            .expect("result should be an array");
        assert_eq!(txs.len(), 1, "Expected one transaction, got:\n{output}");
        assert_eq!(
            txs[0]["to"].as_str(),
            Some("0x7290f841536a3f73835ffad72d27b8c905e1b497"),
            "Expected CREATE destination address in envelope, got:\n{output}"
        );
        assert_eq!(
            txs[0]["signature"].as_str(),
            Some("CREATE()"),
            "Expected CREATE signature in envelope, got:\n{output}"
        );
        assert_eq!(
            json["query"]["command"].as_str(),
            Some("tx"),
            "Expected tx query metadata in envelope, got:\n{output}"
        );
    }

    // cargo run --bin mevlog search -b 23305021:23305023 --rpc-url $ETH_RPC_URL --format json-pretty --sort 'erc20Transfer|0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
    #[test]
    fn test_cli_format_search_erc20_transfer() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
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

        assert!(
            !output.contains("\"logs\":"),
            "Expected logs to be omitted by default in json format.\n\nGot:\n{output}",
        );
    }

    // cargo run --bin mevlog search --to 0x9008D19f58AAbD9eD0D60971565AA8510560ab41 -b 23632775:23632875 --sort 'erc20Transfer|0x6982508145454ce325ddbe47a25d4ec3d2311933' --limit 1 --chain-id 1 --format json --rpc-url $ETH_RPC_URL
    #[test]
    fn test_cli_erc20_transfer_filters() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("search")
            .arg("--to")
            .arg("0x9008D19f58AAbD9eD0D60971565AA8510560ab41")
            .arg("-b")
            .arg("23632775:23632875")
            .arg("--sort")
            .arg("erc20Transfer|0x6982508145454ce325ddbe47a25d4ec3d2311933")
            .arg("--limit")
            .arg("1")
            .arg("--chain-id")
            .arg("1")
            .arg("--format")
            .arg("json")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
        let txs = json["result"]
            .as_array()
            .expect("result should be an array");
        assert!(txs.is_empty(), "Expected empty txs array, got:\n{output}");
    }

    // cargo run --bin mevlog tx 0x71b78307c2e604576efe962cc49e1b64f69409aac5eef0466302add48fe25b0e --rpc-url $ETH_RPC_URL --evm-ops --evm-trace revm --format json-pretty
    #[test]
    fn test_cli_opcodes_tracing() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("tx")
            .arg("0x71b78307c2e604576efe962cc49e1b64f69409aac5eef0466302add48fe25b0e")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .arg("--evm-ops")
            .arg("--evm-trace")
            .arg("revm")
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
        let txs = json["result"]
            .as_array()
            .expect("result should be an array");
        assert!(!txs.is_empty(), "Expected at least one transaction");
        assert!(
            txs[0].get("evm_opcodes").is_some(),
            "Expected evm_opcodes field in transaction:\n{output}"
        );
        let opcodes = txs[0]["evm_opcodes"]
            .as_array()
            .expect("evm_opcodes should be an array");
        assert!(!opcodes.is_empty(), "Expected non-empty evm_opcodes array");
        assert_eq!(
            opcodes[0]["op"].as_str(),
            Some("PUSH1"),
            "Expected first opcode to be PUSH1"
        );
    }

    // cargo run --bin mevlog tx 0xe7657d9eac810efacf20a1715013edb02f7811270f11feaa040ded37c8ec2bd9 --rpc-url $BASE_RPC_URL --evm-ops --evm-trace revm --format json-pretty
    #[test]
    fn test_op_stack_opcodes_tracing() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("tx")
            .arg("0xe7657d9eac810efacf20a1715013edb02f7811270f11feaa040ded37c8ec2bd9")
            .arg("--rpc-url")
            .arg(std::env::var("BASE_RPC_URL").expect("BASE_RPC_URL must be set"))
            .arg("--evm-ops")
            .arg("--evm-trace")
            .arg("revm")
            .arg("--format")
            .arg("json-pretty")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
        let txs = json["result"]
            .as_array()
            .expect("result should be an array");
        assert!(!txs.is_empty(), "Expected at least one transaction");
        assert!(
            txs[0].get("evm_opcodes").is_some(),
            "Expected evm_opcodes field in transaction:\n{output}"
        );
        let opcodes = txs[0]["evm_opcodes"]
            .as_array()
            .expect("evm_opcodes should be an array");
        assert!(!opcodes.is_empty(), "Expected non-empty evm_opcodes array");
        assert_eq!(
            opcodes[0]["op"].as_str(),
            Some("PUSH1"),
            "Expected first opcode to be PUSH1"
        );
    }

    // cargo run --bin mevlog debug-available --rpc-url $ETH_RPC_URL
    #[test]
    fn test_cli_debug_available_true() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("debug-available")
            .arg("--rpc-url")
            .arg(std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set"))
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        assert!(output.trim() == "true", "Expected: true\n\nGot:\n{output}");
    }

    // cargo run --bin mevlog debug-available --rpc-url https://eth.merkle.io
    #[test]
    fn test_cli_debug_available_false() {
        let cmd = Command::new("cargo")
            .env("RUST_LOG", "off")
            .arg("run")
            .arg("--bin")
            .arg("mevlog")
            .arg("debug-available")
            .arg("--rpc-url")
            .arg("https://eth.merkle.io")
            .output()
            .expect("failed to execute CLI");

        let output = String::from_utf8(cmd.stdout).unwrap();
        assert!(
            output.trim() == "false",
            "Expected: false\n\nGot:\n{output}"
        );
    }
}
