# IPFS Uploads

The `--ipfs` global flag uploads the rendered query output to [IPFS](https://ipfs.tech/) and prints a CID + gateway URL instead of producing local output. It works on the query commands (`query`, `tx`, `tx-logs`, `block`, `block-txs`, `block-logs`) with any `--format`, so you can share a query result - including the self-contained static HTML page (pure HTML + CSS, no JavaScript) - as a permanent link.

> **Uploads are public and effectively permanent.** Anyone with the CID can fetch the content, and IPFS has no reliable delete. Do not upload results you would not publish.

## Usage

```bash
# Share a query result as an HTML page
mevlog query -b 100:latest \
  --sql "SELECT block_number, tx_hash, format_ether(value) AS eth FROM transactions ORDER BY value DESC LIMIT 20" \
  --desc "Top 20 transfers, last 100 blocks" \
  --format html --ipfs
```

```text
Uploaded to IPFS
  cid:     bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli
  gateway: https://ipfs.io/ipfs/bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli
  pinata:  https://example-123.mypinata.cloud/ipfs/bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli
  file:    mevlog-4f1d1f70415e4d75.html
```

With `--format json` / `json-pretty` the receipt is printed as JSON instead:

```json
{
  "cid": "bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli",
  "gateway_url": "https://ipfs.io/ipfs/bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli",
  "pinata_gateway_url": "https://example-123.mypinata.cloud/ipfs/bafybeib36krhffuh3jkcv2uubvyhnhkfjbu7f3sciqkrnriimtrbdcnzli",
  "filename": "mevlog-4f1d1f70415e4d75.json"
}
```

`pinata_gateway_url` is `null` when the dedicated gateway domain is unknown, and always on the `kubo` backend.

## What gets uploaded

The exact bytes the `--format` would have produced locally: the JSON `QueryResponse` envelope (`.json`), the CSV rows (`.csv`), the plain-text table (`.txt`) or the self-contained HTML page (`.html`). The object is always named `mevlog-<content-hash>.<ext>` (`--html-filename` is ignored). The hash covers chain + query + description + columns + rows, so an identical result maps to the same filename, and `--desc` changes it.

## Backends

The backend is selected by the `[ipfs]` block in `~/.mevlog/config.toml`:

- **`pinata`** (default) - uploads to the managed [Pinata](https://pinata.cloud) pinning service. Persistent link; needs an API JWT with the `Files: Write` scope, via `ipfs.pinata_jwt` or the `MEVLOG_PINATA_JWT` env var (the env var wins).
- **`kubo`** - adds the file to a local IPFS daemon via `/api/v0/add`. No account needed, but requires a running `ipfs daemon`, and the content is only reachable while your node (or another node that pinned it) stays online.

See [config.toml](./config.md#ipfs---ipfs-uploads---ipfs) for the full key reference, including the Pinata JWT scopes.

## Gateways

The printed `gateway` URL uses a public gateway (default `https://ipfs.io`, overridable via `ipfs.gateway`). Public gateways must first discover a fresh CID via the IPFS DHT, which can take minutes.

On the `pinata` backend the receipt additionally carries your account's dedicated gateway URL, which serves the upload immediately. The domain comes from `ipfs.pinata_gateway` / the `MEVLOG_PINATA_GATEWAY` env var, or is auto-discovered via the Pinata API when the JWT also has the `Gateways: Read` scope; without either, uploads still work but only the public gateway URL is reported.

## MCP

The MCP server exposes the same functionality as the [`upload_query` tool](./mcp.md#upload_query), so LLM agents can publish query results and hand back a shareable link.
