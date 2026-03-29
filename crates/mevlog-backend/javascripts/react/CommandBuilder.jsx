import React from 'react';

const CommandBuilder = ({
  type = 'search', // 'search' or 'explore'
  params = {},
  style = {}
}) => {
  const buildCommand = () => {
    if (type === 'explore') {
      return buildExploreCommand(params);
    } else {
      return buildSearchCommand(params);
    }
  };

  const buildExploreCommand = (params) => {
    let command = 'mevlog search';

    // Block number - only show 'latest' when changing chains or no block number is known
    // For block navigation, show the immediate target block number
    const blockNumber = (params.block_number && !params.isChangingChain) ? params.block_number : 'latest';
    command += ` -b ${blockNumber}`;

    // Chain ID
    if (params.chain_id) {
      command += ` --chain-id ${params.chain_id}`;
    }

    return command;
  };

  const buildSearchCommand = (params) => {
    let command = 'mevlog search';

    // Blocks (with default)
    const blocks = params.blocks || 'latest:latest';
    command += ` -b ${blocks}`;

    // Position (only if provided)
    if (params.position && params.position.trim()) {
      command += ` -p ${params.position}`;
    }

    // Chain ID
    if (params.chain_id) {
      command += ` --chain-id ${params.chain_id}`;
    }

    // From address
    if (params.from && params.from.trim()) {
      command += ` --from ${params.from}`;
    }

    // To address
    if (params.to && params.to.trim()) {
      command += ` --to ${params.to}`;
    }

    // Event filter
    if (params.event && params.event.trim()) {
      command += ` --event "${params.event}"`;
    }

    // Not event filter
    if (params.not_event && params.not_event.trim()) {
      command += ` --not-event "${params.not_event}"`;
    }

    // Method filter
    if (params.method && params.method.trim()) {
      command += ` --method "${params.method}"`;
    }

    // ERC20 Transfer filter
    if (params.erc20_transfer && params.erc20_transfer.trim()) {
      command += ` --erc20-transfer "${params.erc20_transfer}"`;
    }

    // Transaction cost filter
    if (params.tx_cost && params.tx_cost.trim()) {
      command += ` --tx-cost "${params.tx_cost}"`;
    }


    // Gas price filter
    if (params.gas_price && params.gas_price.trim()) {
      command += ` --gas-price "${params.gas_price}"`;
    }



    return command;
  };

  const defaultStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '16px',
    fontFamily: 'monospace',
    fontSize: '14px'
  };

  const linkStyle = {
    color: '#fff',
    textDecoration: 'none',
    marginRight: '8px'
  };

  const commandStyle = {
    color: '#ffd700',
    fontWeight: 'normal'
  };

  const combinedStyle = { ...defaultStyle, ...style };

  return (
    <div style={combinedStyle}>
      <a
        href="https://github.com/pawurb/mevlog-rs"
        target="_blank"
        rel="noopener noreferrer"
        style={linkStyle}
      >
        CLI
      </a>
      : <span style={commandStyle}>{buildCommand()}</span>
    </div>
  );
};

export default CommandBuilder;