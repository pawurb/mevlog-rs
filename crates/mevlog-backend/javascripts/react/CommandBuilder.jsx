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
    // Explore renders a single block's transactions via `block-txs`.
    const blockNumber = (params.block_number && !params.isChangingChain) ? params.block_number : 'latest';
    let command = `mevlog block-txs ${blockNumber}`;

    if (params.chain_id) {
      command += ` --chain-id ${params.chain_id}`;
    }

    return command;
  };

  const buildSearchCommand = (params) => {
    // Search runs read-only SQL over the indexed block range via `query`.
    const blocks = params.blocks || 'latest';
    let command = `mevlog query -b ${blocks}`;

    if (params.chain_id) {
      command += ` --chain-id ${params.chain_id}`;
    }

    if (params.sql && params.sql.trim()) {
      command += ` --sql "${params.sql.trim()}"`;
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