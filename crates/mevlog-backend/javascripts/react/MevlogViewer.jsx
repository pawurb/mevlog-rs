import React, { useState, useEffect } from 'react';
import Transaction, { TransactionTableHeader } from './Transaction';

const MevlogViewer = ({ replaceMode = false, showBlockNumbers = true, chainData: externalChainData = null, waitForExternalChainData = false }) => {
  const [transactions, setTransactions] = useState([]);
  const [chainData, setChainData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [sortConfig, setSortConfig] = useState({ key: null, direction: 'asc' });
  const [error, setError] = useState(null);

  const handleSort = (key) => {
    if (sortConfig.key === key) {
      if (sortConfig.direction === 'asc') {
        setSortConfig({ key, direction: 'desc' });
      } else if (sortConfig.direction === 'desc') {
        setSortConfig({ key: null, direction: 'asc' }); // Disable sorting
      }
    } else {
      setSortConfig({ key, direction: 'asc' });
    }
  };

  const getSortedTransactions = () => {
    return [...transactions].sort((a, b) => {
      // If no custom sorting, use default: block number descending, then tx index ascending
      if (!sortConfig.key) {
        const blockDiff = b.block_number - a.block_number;
        if (blockDiff !== 0) return blockDiff;
        return a.index - b.index;
      }

      // Custom sorting
      let aValue, bValue;

      if (sortConfig.key === 'gas_price') {
        aValue = parseFloat(a.gas_price) || 0;
        bValue = parseFloat(b.gas_price) || 0;
      } else if (sortConfig.key === 'tx_cost') {
        aValue = parseFloat(a.tx_cost) || 0;
        bValue = parseFloat(b.tx_cost) || 0;
      } else if (sortConfig.key === 'index') {
        aValue = parseInt(a.index) || 0;
        bValue = parseInt(b.index) || 0;
      }

      if (aValue < bValue) {
        return sortConfig.direction === 'asc' ? -1 : 1;
      }
      if (aValue > bValue) {
        return sortConfig.direction === 'asc' ? 1 : -1;
      }
      return 0;
    });
  };

  // Load chain data on component mount or when external chain data changes
  useEffect(() => {
    if (externalChainData) {
      // Check if external chain data contains an error
      if (externalChainData.error) {
        setError(externalChainData.error);
        setChainData(null);
      } else {
        // Use external chain data (from ExploreViewer)
        setChainData(externalChainData);
        setError(null); // Clear any previous errors
      }
    } else if (!waitForExternalChainData) {
      // Load own chain data (for standalone use only)
      const loadChainData = async () => {
        try {
          // Extract RPC URL from current page URL params if available
          const urlParams = new URLSearchParams(window.location.search);
          const rpcUrl = urlParams.get('rpc_url');
          const chainId = urlParams.get('chain_id');

          let chainInfoUrl = '/api/chain-info';
          const queryParams = [];

          if (rpcUrl) {
            queryParams.push(`rpc_url=${encodeURIComponent(rpcUrl)}`);
          }

          if (chainId) {
            queryParams.push(`chain_id=${chainId}`);
          } else {
            // Default to chain_id 1 (Ethereum) if neither is provided
            queryParams.push('chain_id=1');
          }

          if (queryParams.length > 0) {
            chainInfoUrl += `?${queryParams.join('&')}`;
          }

          const response = await fetch(chainInfoUrl);
          if (response.ok) {
            const data = await response.json();
            setChainData(data);
          } else {
            try {
              // Chain-info controller returns the error as a JSON string, not an object
              const errorMessage = await response.json();
              const errorText = typeof errorMessage === 'string' ? errorMessage : `Failed to load chain data: ${response.status} ${response.statusText}`;
              console.error(errorText);
              setError(errorText);
            } catch (parseError) {
              const errorText = `Failed to load chain data: ${response.status} ${response.statusText}`;
              console.error(errorText);
              setError(errorText);
            }
          }
        } catch (error) {
          const errorText = `Failed to load chain data: ${error.message}`;
          console.error(errorText);
          setError(errorText);
        }
      };
      loadChainData();
    }
    // If waiting for external chain data but none provided, wait
  }, [externalChainData, waitForExternalChainData]);

  // Update existing transactions when chain data changes
  useEffect(() => {
    if (chainData && transactions.length > 0) {
      setTransactions(prevTransactions =>
        prevTransactions.map(tx => ({
          ...tx,
          chain_id: chainData.chain_id,
          chain_name: chainData.name,
          explorer_url: chainData.explorer_url,
          native_token_price: chainData.current_token_price
        }))
      );
    }
  }, [chainData]);

  const updateTransactions = (jsonData, replace = false) => {
    // Check if data contains an error
    if (jsonData && jsonData.error) {
      setError(jsonData.error);
      setLoading(false);
      return;
    }

    // Clear any previous errors
    setError(null);

    setTransactions(prevTransactions => {
      let newTransactions;

      // Handle both old and new data structures
      if (Array.isArray(jsonData)) {
        // New format: direct array of transactions
        newTransactions = jsonData.map(tx => ({
          ...tx,
          // Add chain data if available
          ...(chainData && {
            chain_id: chainData.chain_id,
            chain_name: chainData.name,
            explorer_url: chainData.explorer_url,
            native_token_price: chainData.current_token_price
          })
        }));
      } else {
        // Single transaction object
        newTransactions = [{
          ...jsonData,
          ...(chainData && {
            chain_id: chainData.chain_id,
            chain_name: chainData.name,
            explorer_url: chainData.explorer_url,
            native_token_price: chainData.current_token_price
          })
        }];
      }

      if (replace || replaceMode) {
        // Replace mode: completely replace existing transactions
        return newTransactions;
      } else {
        // Append mode: remove duplicates and append to end
        const newTxHashes = new Set(newTransactions.map(tx => tx.tx_hash));
        const filteredPrevTransactions = prevTransactions.filter(tx => !newTxHashes.has(tx.tx_hash));
        return [...filteredPrevTransactions, ...newTransactions];
      }
    });
    setLoading(false);
  };

  const clearTransactions = () => {
    setTransactions([]);
    setLoading(true);
    setError(null);
  };

  useEffect(() => {
    // Expose update functions to global scope
    window.updateMevlogViewer = (jsonData) => updateTransactions(jsonData, false);
    window.replaceMevlogViewer = (jsonData) => updateTransactions(jsonData, true);
    window.clearMevlogViewer = clearTransactions;

    return () => {
      delete window.updateMevlogViewer;
      delete window.replaceMevlogViewer;
      delete window.clearMevlogViewer;
    };
  }, [chainData]);

  // Error popup styling
  const alertStyle = {
    position: 'fixed',
    top: '20px',
    right: '20px',
    backgroundColor: '#f8d7da',
    color: '#721c24',
    border: '1px solid #f5c6cb',
    borderRadius: '4px',
    padding: '8px 12px',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    zIndex: 1001,
    maxWidth: '400px',
    boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
    transition: 'opacity 0.3s ease'
  };

  const closeButtonStyle = {
    background: 'none',
    border: 'none',
    color: '#721c24',
    cursor: 'pointer',
    fontSize: '16px',
    fontWeight: 'bold',
    padding: '0',
    marginLeft: '12px'
  };

  return (
    <div className="mevlog-viewer">
      {/* Error popup - always render if error exists */}
      {error && (
        <div style={alertStyle}>
          <span>{error}</span>
          <button
            onClick={() => setError(null)}
            style={closeButtonStyle}
            title="Close"
          >
            ×
          </button>
        </div>
      )}

      {loading ? (
        <div className="loading"></div>
      ) : transactions.length === 0 ? (
        <div className="no-data">Query returned no results</div>
      ) : (
        <>
          <TransactionTableHeader
            sortConfig={sortConfig}
            onSort={handleSort}
            showBlockNumbers={showBlockNumbers}
          />
          {getSortedTransactions().map((transaction) => (
            <Transaction
              key={`${transaction.block_number}-${transaction.tx_hash}`}
              transaction={transaction}
              explorerUrl={transaction.explorer_url || (chainData && chainData.explorer_url)}
              showBlockNumbers={showBlockNumbers}
            />
          ))}
        </>
      )}
    </div>
  );
};

export default MevlogViewer;