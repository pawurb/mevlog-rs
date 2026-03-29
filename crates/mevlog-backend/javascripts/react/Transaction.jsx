import React, { useState } from 'react';

const formatExplorerLink = (address, type, explorerUrl, ensName = null) => {
  if (!address || address === '<Unknown>') return address;
  return (
    <a
      href={`${explorerUrl}/${type}/${address}`}
      target="_blank"
      rel="noopener noreferrer"
      style={{ color: '#4a9eff', textDecoration: 'none' }}
    >
      {ensName || address}
    </a>
  );
};

const formatGasPriceToGwei = (gasPriceWei) => {
  if (!gasPriceWei) return '0';
  return (gasPriceWei / 1e9).toFixed(2);
};

const TransactionTableHeader = ({ sortConfig, onSort, showBlockNumbers = true }) => {
  const headerStyle = {
    display: 'flex',
    alignItems: 'center',
    padding: '4px 8px',
    borderBottom: '2px solid #333',
    backgroundColor: '#2a2a2a',
    fontWeight: 'bold',
    fontSize: '14px',
    color: '#fff'
  };

  const buttonPlaceholderStyle = {
    width: '32px',
    marginRight: '8px',
    flexShrink: 0
  };

  const clickableHeaderStyle = {
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    userSelect: 'none'
  };

  const getSortTriangle = (key) => {
    if (!sortConfig || sortConfig.key !== key) {
      return <span style={{ color: '#666', marginLeft: '2px', fontSize: '12px' }}>▲▼</span>;
    }
    return (
      <span style={{ color: '#fff', marginLeft: '2px', fontSize: '12px' }}>
        {sortConfig.direction === 'asc' ? '▲' : '▼'}
      </span>
    );
  };

  return (
    <div className="transaction-header" style={headerStyle}>
      <div style={buttonPlaceholderStyle}></div>
      {showBlockNumbers && <span style={{ marginRight: '10px', fontSize: '14px', width: '80px', flexShrink: 0 }}>Block</span>}
      <span
        style={{ marginRight: '10px', fontSize: '14px', width: '50px', flexShrink: 0, ...clickableHeaderStyle }}
        onClick={() => onSort && onSort('index')}
      >
        <span>Index</span>
        {getSortTriangle('index')}
      </span>
      <span style={{ marginRight: '10px', fontSize: '14px', width: '120px', flexShrink: 0 }}>Hash</span>
      <span style={{ marginRight: '10px', fontSize: '14px', flex: 1 }}>Signature</span>
      <span
        style={{ marginRight: '8px', fontSize: '14px', width: '110px', flexShrink: 0, ...clickableHeaderStyle, whiteSpace: 'nowrap' }}
        onClick={() => onSort && onSort('gas_price')}
      >
        <span>Gas Price</span>
        {getSortTriangle('gas_price')}
      </span>
      <span
        style={{ marginRight: '8px', fontSize: '14px', width: '100px', flexShrink: 0, ...clickableHeaderStyle, whiteSpace: 'nowrap' }}
        onClick={() => onSort && onSort('tx_cost')}
      >
        <span>Gas Cost</span>
        {getSortTriangle('tx_cost')}
      </span>
      <span style={{ fontSize: '14px', width: '60px', flexShrink: 0 }}>Status</span>
    </div>
  );
};

const TransactionDetails = ({ transaction, explorerUrl, showExtraDetails = true }) => {
  const formatAddress = (address) => {
    if (!address) return '<Unknown>';
    return address;
  };

  const formatHexData = (data) => {
    if (!data) return '';
    // Break long hex strings into multiple lines for readability
    const chunks = data.match(/.{1,64}/g) || [];
    return chunks.map((chunk, idx) => (
      <div key={idx} style={{ fontFamily: 'monospace', color: '#666', marginLeft: '20px' }}>
        {chunk}
      </div>
    ));
  };

  const renderLogEntry = (log, logIndex) => {
    return (
      <div key={logIndex} style={{ marginBottom: '20px' }}>
        <div style={{ color: '#4a9eff', marginBottom: '8px' }}>
          {formatExplorerLink(log.address, 'address', explorerUrl)}
        </div>
        <div style={{ color: '#ffa500', marginBottom: '8px' }}>
          emit {log.signature}
        </div>
        {log.topics && log.topics.map((topic, topicIdx) => (
          <div key={topicIdx} style={{ fontFamily: 'monospace', color: '#666', marginLeft: '20px' }}>
            {topic}
          </div>
        ))}
        {log.data && formatHexData(log.data)}
      </div>
    );
  };

  const detailsStyle = {
    padding: '16px',
    backgroundColor: '#1a1a1a',
    color: '#fff',
    fontFamily: 'monospace',
    fontSize: '14px',
    lineHeight: '1.4',
    borderBottom: '1px solid #333'
  };

  const labelStyle = {
    color: '#4CAF50',
    marginRight: '8px'
  };

  const valueStyle = {
    color: '#fff'
  };

  const separatorStyle = {
    borderTop: '1px dashed #666',
    margin: '16px 0',
    height: '1px'
  };
  console.log(transaction);

  return (
    <div className="tx-details" style={detailsStyle}>
      {/* Transaction Details */}
      <div style={{ color: '#ffa500', marginBottom: '8px' }}>
        <span className="method" style={{ fontWeight: 'bold' }}>{transaction.signature}</span>
      </div>
      <div style={{ marginBottom: '6px' }}>
        {formatExplorerLink(transaction.from, 'address', explorerUrl, transaction.from_ens)} => {formatExplorerLink(transaction.to, 'address', explorerUrl) || '<Unknown>'}
      </div>
      <div style={{ marginBottom: '6px' }}>
        <span style={labelStyle}>Gas Tx Cost:</span>
        <span style={valueStyle}>
          {transaction.display_tx_cost}
          {transaction.display_tx_cost_usd && ` | ${transaction.display_tx_cost_usd}`}
        </span>
      </div>
      <div style={{ marginBottom: '6px' }}>
        <span style={labelStyle}>Gas Price:</span>
        <span style={valueStyle}>{formatGasPriceToGwei(transaction.gas_price)} gwei</span>
      </div>
      <div style={{ marginBottom: '6px' }}>
        <span style={labelStyle}>Gas Used:</span>
        <span style={valueStyle}>{transaction.gas_used || '0'}</span>
      </div>
      <div style={{ marginBottom: '6px' }}>
        <span style={labelStyle}>Real Gas Price:</span>
        <span style={valueStyle}>{formatGasPriceToGwei(transaction.gas_price)} GWEI</span>
      </div>
      <div style={{ marginBottom: '6px' }}>
        <span style={labelStyle}>Value:</span>
        <span style={valueStyle}>{transaction.display_value}</span>
      </div>
      {showExtraDetails && (
        <>
        </>
      )}

      <div style={separatorStyle}></div>

      {/* Contract Events/Logs */}
      {transaction.log_groups && transaction.log_groups.length > 0 && (
        <div>
          {transaction.log_groups.map((logGroup, groupIdx) => (
            <div key={groupIdx} style={{ marginBottom: '20px' }}>
              {/* Display source only once at the top */}
              {logGroup.logs.length > 0 && (
                <div style={{ color: '#4a9eff', marginBottom: '8px' }}>
                  {formatExplorerLink(logGroup.logs[0].source, 'address', explorerUrl)}
                </div>
              )}
              {/* Display events data */}
              {logGroup.logs.map((log, logIdx) => (
                <div key={`${groupIdx}-${logIdx}`} style={{ marginBottom: '8px' }}>
                  <div style={{ color: '#ffa500', marginBottom: '4px' }}>
                    emit {log.signature} {log.symbol || ''}
                  </div>
                  {log.topics && log.topics.map((topic, topicIdx) => (
                    <div key={topicIdx} style={{ fontFamily: 'monospace', color: '#666', marginLeft: '20px' }}>
                      {topic}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          ))}
          <div style={separatorStyle}></div>
        </div>
      )}
    </div>
  );
};

const Transaction = ({ transaction, explorerUrl, showBlockNumbers = true, showExtraDetails = true }) => {
  const [isExpanded, setIsExpanded] = useState(false);

  const toggleExpanded = () => {
    setIsExpanded(!isExpanded);
  };

  let signatureLength = 40;

  const truncatedHash = transaction.tx_hash.substring(0, 10);
  const truncatedSignature = transaction.signature && transaction.signature.length > signatureLength
    ? transaction.signature.substring(0, signatureLength) + '...'
    : transaction.signature;

  const buttonStyle = {
    backgroundColor: '#ffd700',
    border: '1px solid #ccc',
    borderRadius: '4px',
    cursor: 'pointer',
    fontSize: '14px',
    fontWeight: 'bold',
    padding: '6px 10px',
    marginRight: '8px',
    minWidth: '32px',
    height: '32px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    flexShrink: 0
  };

  const rowStyle = {
    display: 'flex',
    alignItems: 'center',
    padding: '4px 8px',
    borderBottom: '1px solid #eee'
  };

  return (
    <>
      <div className="transaction-row" style={rowStyle}>
        <button onClick={toggleExpanded} style={buttonStyle}>
          {isExpanded ? '▲' : '▼'}
        </button>
        {showBlockNumbers && (
          <span className="block-number" style={{ marginRight: '10px', fontSize: '14px', width: '80px', flexShrink: 0 }}>
            <a
              href={`${explorerUrl}/block/${transaction.block_number}`}
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: '#4a9eff', textDecoration: 'none' }}
            >
              #{transaction.block_number}
            </a>
          </span>
        )}
        <span className="tx-index" style={{ marginRight: '10px', color: '#999', fontSize: '14px', width: '50px', flexShrink: 0 }}>
          {transaction.index}:
        </span>
        <span className="tx-hash-short" style={{
          fontFamily: 'monospace',
          marginRight: '10px',
          width: isExpanded ? 'auto' : '120px',
          flex: isExpanded ? 1 : '0 0 auto',
          flexShrink: 0,
          overflow: isExpanded ? 'visible' : 'hidden',
          whiteSpace: 'nowrap'
        }}>
          {isExpanded ? formatExplorerLink(transaction.tx_hash, 'tx', explorerUrl) : (
            <a
              href={`${explorerUrl}/tx/${transaction.tx_hash}`}
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: '#4a9eff', textDecoration: 'none' }}
            >
              {`${truncatedHash}...`}
            </a>
          )}
        </span>
        {!isExpanded && (
          <span className="method" style={{ marginRight: '10px', fontWeight: 'bold', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {truncatedSignature}
          </span>
        )}
        <span className="gas-price" style={{ marginRight: '8px', color: '#4CAF50', width: '110px', flexShrink: 0 }}>
          {formatGasPriceToGwei(transaction.gas_price) + ' gwei' || '0 gwei'}
        </span>
        <span className="tx-cost-usd" style={{ marginRight: '8px', color: '#4CAF50', width: '100px', flexShrink: 0 }}>
          {transaction.display_tx_cost_usd || '$0'}
        </span>
        <span className={`status ${transaction.success ? 'success' : 'failed'}`} style={{ width: '60px', flexShrink: 0 }}>
          {transaction.success ? '✓' : '✗'}
        </span>
      </div>
      {isExpanded && <TransactionDetails transaction={transaction} explorerUrl={explorerUrl} showExtraDetails={showExtraDetails} />}
    </>
  );
};

export default Transaction;
export { TransactionTableHeader };