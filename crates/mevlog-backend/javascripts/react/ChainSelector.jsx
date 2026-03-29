import React, { useState, useEffect, useRef } from 'react';

// Default popular chains to show when search is empty
const DEFAULT_CHAINS = [
  { "chain_id": 1, "name": "Ethereum Mainnet", "chain": "ETH" },
  { "chain_id": 10, "name": "OP Mainnet", "chain": "ETH" },
  { "chain_id": 56, "name": "BNB Smart Chain Mainnet", "chain": "BSC" },
  { "chain_id": 130, "name": "Unichain", "chain": "ETH" },
  { "chain_id": 137, "name": "Polygon Mainnet", "chain": "Polygon" },
  { "chain_id": 324, "name": "zkSync Mainnet", "chain": "ETH" },
  { "chain_id": 8453, "name": "Base", "chain": "ETH" },
  { "chain_id": 42161, "name": "Arbitrum One", "chain": "ETH" },
  { "chain_id": 43114, "name": "Avalanche C-Chain", "chain": "AVAX" },
  { "chain_id": 534352, "name": "Scroll Mainnet", "chain": "ETH" }
];

const ChainSelector = ({ onChainChange, initialChainId = null, disabled = false }) => {
  const [availableChains, setAvailableChains] = useState(DEFAULT_CHAINS);
  const [chainQuery, setChainQuery] = useState('');
  const [selectedChainId, setSelectedChainId] = useState(null);
  const [showDropdown, setShowDropdown] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const inputRef = useRef(null);
  const dropdownRef = useRef(null);


  const fetchAvailableChains = async ({ filter = null, chainId = null } = {}) => {
    try {
      let url = '/api/chains';
      const params = new URLSearchParams();
      if (chainId !== null && chainId !== undefined && chainId !== '') {
        // Send chain_id as an array of integers
        params.append('chain_id', chainId);
      } else if (filter && filter.length >= 2) {
        params.append('filter', filter);
      }

      if ([...params.keys()].length > 0) {
        url += `?${params.toString()}`;
      }

      const response = await fetch(url);
      if (response.ok) {
        const chains = await response.json();
        setAvailableChains(chains);
        if (chains.length > 0) {
          setShowDropdown(true);
        }
      } else {
        console.error('Failed to load available chains:', response.status);
      }
    } catch (error) {
      console.error('Failed to load available chains:', error);
    }
  };

  const selectChain = (chain) => {

    setSelectedChainId(chain.chain_id);
    setChainQuery(`${chain.name} (ID: ${chain.chain_id})`);
    setShowDropdown(false);
    setFocusedIndex(-1);
    if (onChainChange) {
      onChainChange(chain.chain_id);
    }
  };

  const handleInputChange = (event) => {
    const value = event.target.value;
    setChainQuery(value);
    setFocusedIndex(-1);

    // If user clears the input, reset to default chains
    if (!value.trim()) {
      setSelectedChainId(null);
      setAvailableChains(DEFAULT_CHAINS);
      setShowDropdown(true);
      return;
    }

    // Don't automatically switch chains - just fetch for filtering
    // This applies to both numeric and text input
  };

  const handleInputFocus = () => {
    // If no query, show default chains
    if (!chainQuery.trim()) {
      setAvailableChains(DEFAULT_CHAINS);
    }
    if (availableChains.length > 0) {
      setShowDropdown(true);
    }
  };

  const handleInputBlur = (event) => {
    // Delay hiding dropdown to allow clicking on options
    setTimeout(() => {
      if (!dropdownRef.current?.contains(event.relatedTarget)) {
        setShowDropdown(false);
        setFocusedIndex(-1);
      }
    }, 200);
  };

  const handleKeyDown = (event) => {
    if (!showDropdown || availableChains.length === 0) return;

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        setFocusedIndex(prev =>
          prev < availableChains.length - 1 ? prev + 1 : 0
        );
        break;
      case 'ArrowUp':
        event.preventDefault();
        setFocusedIndex(prev =>
          prev > 0 ? prev - 1 : availableChains.length - 1
        );
        break;
      case 'Enter':
        event.preventDefault();
        if (focusedIndex >= 0 && availableChains[focusedIndex]) {
          selectChain(availableChains[focusedIndex]);
        }
        break;
      case 'Escape':
        setShowDropdown(false);
        setFocusedIndex(-1);
        inputRef.current?.blur();
        break;
    }
  };

  useEffect(() => {
    // Add 1-second delay for all queries (numeric and text)
    if (chainQuery && !chainQuery.includes('(ID:')) {
      const t = setTimeout(() => {
        if (/^\d+$/.test(chainQuery.trim())) {
          // Numeric query - fetch by chain ID
          const numericId = parseInt(chainQuery.trim(), 10);
          fetchAvailableChains({ chainId: numericId });
        } else if (chainQuery.length >= 2) {
          // Text query with minimum 2 characters
          fetchAvailableChains({ filter: chainQuery });
        } else {
          // Too short – show default chains
          setAvailableChains(DEFAULT_CHAINS);
          setShowDropdown(true);
        }
      }, 1000);
      return () => clearTimeout(t);
    } else if (!chainQuery.trim()) {
      // No query - show default chains immediately
      setAvailableChains(DEFAULT_CHAINS);
    }
  }, [chainQuery]);

  // Handle initial setup - show default chains
  useEffect(() => {
    setAvailableChains(DEFAULT_CHAINS);
  }, []);

  // Update display when available chains change and we have a selected chain
  useEffect(() => {
    if (selectedChainId && availableChains.length > 0 && !chainQuery.includes('(ID:')) {
      const chain = availableChains.find(c => c.chain_id === selectedChainId);
      if (chain) {
        setChainQuery(`${chain.name} (ID: ${chain.chain_id})`);
      }
    }
  }, [availableChains, selectedChainId]);

  const containerStyle = {
    position: 'relative',
    marginBottom: '16px'
  };

  const chainSelectorStyle = {
    backgroundColor: '#2a2a2a',
    border: '1px solid #444',
    borderRadius: '6px',
    padding: '12px 16px',
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
    fontSize: '14px'
  };

  const inputStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '4px',
    color: '#fff',
    fontSize: '14px',
    padding: '8px 12px',
    minWidth: '150px',
    width: '100%',
    cursor: 'text',
    outline: 'none',
    transition: 'border-color 0.2s ease'
  };

  const inputFocusedStyle = {
    ...inputStyle,
    borderColor: '#ffd700'
  };

  const labelStyle = {
    color: '#888',
    fontSize: '12px',
    fontWeight: '500',
    textTransform: 'uppercase',
    letterSpacing: '0.5px',
    minWidth: '60px'
  };

  const dropdownStyle = {
    position: 'absolute',
    top: '100%',
    left: '12px',
    right: '12px',
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderTop: 'none',
    borderBottomLeftRadius: '4px',
    borderBottomRightRadius: '4px',
    maxHeight: '200px',
    overflowY: 'auto',
    zIndex: 1000,
    boxShadow: '0 4px 6px rgba(0, 0, 0, 0.3)'
  };

  const optionStyle = {
    padding: '10px 12px',
    cursor: 'pointer',
    fontSize: '14px',
    color: '#fff',
    borderBottom: '1px solid #333',
    transition: 'background-color 0.1s ease'
  };

  const optionHoverStyle = {
    ...optionStyle,
    backgroundColor: '#2a2a2a'
  };

  const optionFocusedStyle = {
    ...optionStyle,
    backgroundColor: '#ffd700',
    color: '#000'
  };

  return (
    <div style={containerStyle}>
      <div style={chainSelectorStyle}>
        <span style={labelStyle}>Chain</span>
        <input
          ref={inputRef}
          type="text"
          placeholder="Name or ID."
          value={chainQuery}
          onChange={handleInputChange}
          onFocus={handleInputFocus}
          onBlur={handleInputBlur}
          onKeyDown={handleKeyDown}
          style={showDropdown ? inputFocusedStyle : inputStyle}
          disabled={disabled}
          autoComplete="off"
        />
      </div>

      {showDropdown && availableChains.length > 0 && (
        <div ref={dropdownRef} style={dropdownStyle}>
          {availableChains.map((chain, index) => (
            <div
              key={chain.chain_id}
              style={
                index === focusedIndex
                  ? optionFocusedStyle
                  : optionStyle
              }
              onMouseDown={(e) => {
                e.preventDefault();
                selectChain(chain);
              }}
              onMouseEnter={() => setFocusedIndex(index)}
            >
              {chain.name} (ID: {chain.chain_id})
            </div>
          ))}
        </div>
      )}

    </div>
  );
};

export default ChainSelector;