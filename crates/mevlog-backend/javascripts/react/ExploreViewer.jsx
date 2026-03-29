import React, { useState, useEffect } from 'react';
import MevlogViewer from './MevlogViewer';
import ChainSelector from './ChainSelector';
import CommandBuilder from './CommandBuilder';

const ExploreViewer = () => {
  const [loading, setLoading] = useState(true);
  const [currentBlockNumber, setCurrentBlockNumber] = useState(null);
  const [blockData, setBlockData] = useState(null);
  const [chainData, setChainData] = useState(null);
  const [hasInitialData, setHasInitialData] = useState(false);
  const [selectedChainId, setSelectedChainId] = useState(null);
  const [loadingMessage, setLoadingMessage] = useState('Loading transactions...');
  const [isChangingChain, setIsChangingChain] = useState(false);


  const updateURLParams = (chainId = null, blockNumber = null) => {
    const url = new URL(window.location);
    const params = new URLSearchParams(url.search);

    // Only set chain_id if it's not the default (1)
    if (chainId && chainId !== 1) {
      params.set('chain_id', chainId);
    } else {
      params.delete('chain_id');
    }

    // Only set block_number if it's not "latest" or null
    if (blockNumber && blockNumber !== 'latest') {
      params.set('block_number', blockNumber);
    } else {
      params.delete('block_number');
    }

    // Update URL without triggering page reload
    const newUrl = `${url.pathname}${params.toString() ? '?' + params.toString() : ''}`;
    window.history.replaceState({}, '', newUrl);
  };



  const fetchChainData = async (chainId = null) => {
    try {
      // Use provided chainId, or extract from URL params, or default to 1
      const urlParams = new URLSearchParams(window.location.search);
      const rpcUrl = urlParams.get('rpc_url');
      const urlChainId = urlParams.get('chain_id');

      const targetChainId = chainId || urlChainId || selectedChainId || 1;

      let chainInfoUrl = '/api/chain-info';
      const queryParams = [];

      if (rpcUrl) {
        queryParams.push(`rpc_url=${encodeURIComponent(rpcUrl)}`);
      } else {
        queryParams.push(`chain_id=${targetChainId}`);
      }

      if (queryParams.length > 0) {
        chainInfoUrl += `?${queryParams.join('&')}`;
      }

      const response = await fetch(chainInfoUrl);
      if (response.ok) {
        const data = await response.json();
        setChainData(data);
        if (!selectedChainId) {
          setSelectedChainId(data.chain_id);
        }
      } else {
        try {
          // Chain-info controller returns the error as a JSON string, not an object
          const errorMessage = await response.json();
          const errorText = typeof errorMessage === 'string' ? errorMessage : `Failed to load chain data: ${response.status} ${response.statusText}`;
          console.error(errorText);
          setChainData({ error: errorText });
        } catch (parseError) {
          const errorText = `Failed to load chain data: ${response.status} ${response.statusText}`;
          console.error(errorText);
          setChainData({ error: errorText });
        }
      }
    } catch (error) {
      const errorText = `Failed to load chain data: ${error.message}`;
      console.error(errorText);
      setChainData({ error: errorText });
    }
  };

  const fetchExploreData = async (blockNumber = null, chainId = null, skipUrlUpdate = false) => {
    try {
      setLoading(true);

      // Extract RPC URL from current page URL params if available
      const urlParams = new URLSearchParams(window.location.search);
      const rpcUrl = urlParams.get('rpc_url');
      const urlChainId = urlParams.get('chain_id');

      const targetChainId = chainId || urlChainId || selectedChainId || 1;

      let exploreApiUrl = '/api/explore';
      const queryParams = [];

      if (rpcUrl) {
        queryParams.push(`rpc_url=${encodeURIComponent(rpcUrl)}`);
      } else {
        queryParams.push(`chain_id=${targetChainId}`);
      }

      if (blockNumber !== null) {
        queryParams.push(`block_number=${blockNumber}`);
      }

      if (queryParams.length > 0) {
        exploreApiUrl += `?${queryParams.join('&')}`;
      }

      const response = await fetch(exploreApiUrl);
      if (response.ok) {
        const jsonData = await response.json();
        setBlockData(jsonData);
        setHasInitialData(true);

        // Extract block number from the first transaction
        if (jsonData && jsonData.length > 0 && jsonData[0].block_number) {
          const newBlockNumber = jsonData[0].block_number;
          setCurrentBlockNumber(newBlockNumber);
          // Update URL with new block number only if we haven't already set it via navigation
          if (!skipUrlUpdate) {
            updateURLParams(chainId || selectedChainId, newBlockNumber);
          }
        }

        // Update MevlogViewer with the fetched data (replace mode)
        if (window.replaceMevlogViewer) {
          window.replaceMevlogViewer(jsonData);
        }

        setLoading(false);
      } else {
        const errorData = await response.json();
        const errorMessage = errorData.error || `HTTP ${response.status}: ${response.statusText}`;
        setLoading(false);

        // Pass error to MevlogViewer
        if (window.updateMevlogViewer) {
          window.updateMevlogViewer({ error: errorMessage });
        }
      }
    } catch (err) {
      setLoading(false);

      // Pass error to MevlogViewer
      if (window.updateMevlogViewer) {
        window.updateMevlogViewer({ error: `Failed to fetch explore data: ${err.message}` });
      }
    }
  };

  const handlePrevBlock = () => {
    if (currentBlockNumber && currentBlockNumber > 0) {
      const targetBlock = currentBlockNumber - 1;
      setLoadingMessage(`Loading block #${targetBlock}...`);
      fetchExploreData(targetBlock, null, false); // Let fetchExploreData update URL on success
    }
  };

  const handleNextBlock = () => {
    if (currentBlockNumber) {
      const targetBlock = currentBlockNumber + 1;
      setLoadingMessage(`Loading block #${targetBlock}...`);
      fetchExploreData(targetBlock, null, false); // Let fetchExploreData update URL on success
    }
  };

  const handleChainChange = async (newChainId) => {
    setSelectedChainId(newChainId);
    setCurrentBlockNumber(null); // Reset block number so command shows 'latest'
    setLoading(true);
    setIsChangingChain(true); // Flag that we're changing chains
    setLoadingMessage('Switching networks...');

    // Update URL immediately when chain changes
    updateURLParams(newChainId, null);

    try {
      // Fetch new chain data and explore data for the selected chain
      await fetchChainData(newChainId);
      await fetchExploreData(null, newChainId);
    } catch (error) {
      setLoading(false);
    }
    setIsChangingChain(false); // Clear the chain changing flag
  };



  useEffect(() => {
    // Get initial parameters from the explore container data attributes
    const container = document.getElementById('explore-react-root');
    const initialChainId = container?.getAttribute('data-chain-id');
    const initialBlockNumber = container?.getAttribute('data-block-number');

    // Parse parameters and set initial state
    const chainId = initialChainId && initialChainId !== '1' ? parseInt(initialChainId) : null;
    const blockNumber = initialBlockNumber && initialBlockNumber !== 'latest' ? initialBlockNumber : null;


    if (chainId) {
      setSelectedChainId(chainId);
    }

    fetchChainData(chainId);
    fetchExploreData(blockNumber, chainId);
  }, []);


  const navButtonStyle = {
    backgroundColor: '#ffd700',
    border: '1px solid #ccc',
    borderRadius: '4px',
    cursor: 'pointer',
    fontSize: '14px',
    fontWeight: 'bold',
    padding: '8px 16px',
    margin: '0 8px',
    minWidth: '60px',
    height: '36px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: '#000',
    transition: 'all 0.2s ease'
  };

  const disabledButtonStyle = {
    ...navButtonStyle,
    backgroundColor: '#ccc',
    cursor: 'not-allowed',
    opacity: 0.6
  };

  const chainInfoStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '6px',
    padding: '12px 16px',
    marginBottom: '16px',
    display: 'flex',
    flexWrap: 'wrap',
    gap: '24px',
    alignItems: 'center',
    fontSize: '14px'
  };

  const chainMetadataItemStyle = {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px'
  };

  const chainLabelStyle = {
    color: '#888',
    fontSize: '12px',
    fontWeight: '500',
    textTransform: 'uppercase',
    letterSpacing: '0.5px'
  };

  const chainValueStyle = {
    color: '#fff',
    fontSize: '14px',
    fontWeight: '600'
  };

  const placeholderStyle = {
    color: '#666',
    fontSize: '14px',
    fontWeight: '400',
    fontStyle: 'italic'
  };

  const headerStyle = {
    paddingBottom: '8px',
    marginBottom: '8px',
    borderBottom: '1px solid #333'
  };

  const navigationStyle = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '16px',
    marginTop: '8px'
  };

  const currentBlockStyle = {
    fontSize: '16px',
    fontWeight: 'bold',
    color: '#fff',
    padding: '0 16px'
  };



  const loadingOverlayStyle = {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(0, 0, 0, 0.7)',
    display: 'flex',
    alignItems: 'flex-start',
    justifyContent: 'center',
    paddingTop: '60px',
    zIndex: 1000,
    borderRadius: '6px'
  };

  const spinnerStyle = {
    width: '50px',
    height: '50px',
    border: '5px solid #ffffff',
    borderTop: '5px solid #ffd700',
    borderRadius: '50%',
    animation: 'spin 1s linear infinite',
    display: 'block'
  };

  const spinnerContainerStyle = {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: '16px'
  };

  const loadingTextStyle = {
    color: '#ffd700',
    fontSize: '16px',
    fontWeight: '600',
    textAlign: 'center',
    textShadow: '0 0 10px rgba(255, 215, 0, 0.5)'
  };

  const transactionContainerStyle = {
    position: 'relative',
    minHeight: '400px'
  };

  const skeletonStyle = {
    display: !hasInitialData && loading ? 'block' : 'none',
    padding: '20px'
  };

  const skeletonRowStyle = {
    height: '60px',
    backgroundColor: '#2a2a2a',
    borderRadius: '4px',
    marginBottom: '8px',
    display: 'flex',
    alignItems: 'center',
    padding: '0 16px',
    animation: 'pulse 1.5s ease-in-out infinite alternate'
  };

  const skeletonTextStyle = {
    height: '12px',
    backgroundColor: '#444',
    borderRadius: '2px',
    flex: 1,
    marginRight: '16px'
  };

  return (
    <div className="explore-viewer">
      <style>
        {`
          @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
          }
          
          @keyframes pulse {
            0% { opacity: 0.7; }
            50% { opacity: 1; }
            100% { opacity: 0.7; }
          }
        `}
      </style>

      <ChainSelector
        onChainChange={handleChainChange}
        initialChainId={selectedChainId}
        disabled={loading}
      />

      <div style={chainInfoStyle}>
        <div style={chainMetadataItemStyle}>
          <span style={chainLabelStyle}>Network</span>
          <span style={chainData?.name ? chainValueStyle : placeholderStyle}>
            {chainData?.name || '...'}
          </span>
        </div>

        <div style={chainMetadataItemStyle}>
          <span style={chainLabelStyle}>Chain ID</span>
          <span style={chainData?.chain_id ? chainValueStyle : placeholderStyle}>
            {chainData?.chain_id || '...'}
          </span>
        </div>

        <div style={chainMetadataItemStyle}>
          <span style={chainLabelStyle}>Currency</span>
          <span style={chainData?.currency ? chainValueStyle : placeholderStyle}>
            {chainData?.currency || '...'}
          </span>
        </div>

        <div style={chainMetadataItemStyle}>
          <span style={chainLabelStyle}>Explorer</span>
          {chainData?.explorer_url ? (
            <a
              href={chainData.explorer_url}
              target="_blank"
              rel="noopener noreferrer"
              style={{ ...chainValueStyle, color: '#4a9eff', textDecoration: 'none' }}
            >
              {chainData.explorer_url}
            </a>
          ) : (
            <span style={placeholderStyle}>...</span>
          )}
        </div>
      </div>

      <CommandBuilder
        type="explore"
        params={{
          chain_id: selectedChainId,
          block_number: currentBlockNumber,
          loading: loading,
          isChangingChain: isChangingChain
        }}
      />

      <div className="explore-header" style={headerStyle}>
        <div className="block-navigation" style={navigationStyle}>
          {currentBlockNumber && (
            <>
              <button
                onClick={handlePrevBlock}
                disabled={loading || currentBlockNumber <= 0}
                style={loading || currentBlockNumber <= 0 ? disabledButtonStyle : navButtonStyle}
              >
                {'◀'}
              </button>
              <span className="current-block" style={currentBlockStyle}>
                {chainData && chainData.explorer_url ? (
                  <a
                    href={`${chainData.explorer_url}/block/${currentBlockNumber}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{ color: '#4a9eff', textDecoration: 'none' }}
                  >
                    #{currentBlockNumber}
                  </a>
                ) : (
                  `#${currentBlockNumber}`
                )}
              </span>
              <button
                onClick={handleNextBlock}
                disabled={loading}
                style={loading ? disabledButtonStyle : navButtonStyle}
              >
                {'▶'}
              </button>
            </>
          )}
        </div>
      </div>

      <div style={transactionContainerStyle}>
        <div style={skeletonStyle}>
          {[...Array(6)].map((_, i) => (
            <div key={i} style={skeletonRowStyle}>
              <div style={{ ...skeletonTextStyle, width: '40px' }}></div>
              <div style={{ ...skeletonTextStyle, width: '120px' }}></div>
              <div style={{ ...skeletonTextStyle, width: '200px' }}></div>
              <div style={{ ...skeletonTextStyle, width: '80px' }}></div>
              <div style={{ ...skeletonTextStyle, width: '60px' }}></div>
              <div style={{ ...skeletonTextStyle, width: '40px', marginRight: '0' }}></div>
            </div>
          ))}
        </div>
        <MevlogViewer replaceMode={true} showBlockNumbers={false} chainData={chainData} waitForExternalChainData={true} />
        {loading && hasInitialData && (
          <div style={loadingOverlayStyle}>
            <div style={spinnerContainerStyle}>
              <div style={spinnerStyle}></div>
              <div style={{ ...loadingTextStyle, animation: 'pulse 1.5s ease-in-out infinite' }}>
                {loadingMessage}
              </div>
            </div>
          </div>
        )}
      </div>

    </div>
  );
};

export default ExploreViewer;