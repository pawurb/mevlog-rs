https://github.com/pawurb/mevlog-rs
https://mevlog.rs
mevlog search --blocks 23585128:23585168 --sort "erc20Transfer|0x6982508145454ce325ddbe47a25d4ec3d2311933" --limit 1 --chain-id 1 --format json-pretty | head
mevlog tx 0x987aa0b1efdc9bab59dcd2c6842ca57cf5f611f9360b4721020167dd19d6cea0
mevlog search --blocks 23585148:23585168 --event 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 --event 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 --event "/(?i)(sync).+/" --erc20-transfer "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48|ge10gwei"
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --chain-id 1
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --chain-id 1 --trace rpc
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --chain-id 1 --trace revm
mevlog search --help
https://chainlist.org/
mevlog chains
mevlog chains --filter poly
unset ETH_RPC_URL
mevlog chain-info --chain-id 137
https://mevlog.rs
https://ratatui.rs/
https://ratatui.rs/examples/apps/
