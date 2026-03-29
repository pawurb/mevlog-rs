import React, { useState, useEffect } from 'react';
import ChainSelector from './ChainSelector';
import CommandBuilder from './CommandBuilder';

const SearchForm = ({ initialValues = {}, onSubmit }) => {
  const [formData, setFormData] = useState({
    blocks: initialValues.blocks || '',
    position: initialValues.position || '',
    from: initialValues.from || '',
    to: initialValues.to || '',
    event: initialValues.event || '',
    not_event: initialValues.not_event || '',
    method: initialValues.method || '',
    erc20_transfer: initialValues.erc20_transfer || '',
    tx_cost: initialValues.tx_cost || '',
    gas_price: initialValues.gas_price || '',
    chain_id: initialValues.chain_id || ''
  });



  // Initialize results area and hide loading spinner
  useEffect(() => {
    const cmdOutput = document.querySelector('.js-cmd-output');
    if (cmdOutput) {
      // If the content doesn't already contain our placeholder, set it
      if (!cmdOutput.innerHTML.includes('Press search to query') && !cmdOutput.innerHTML.trim()) {
        cmdOutput.innerHTML = '<div style="color: #888; padding: 20px; text-align: center; font-family: monospace;">Press search to query</div>';
      }
      cmdOutput.style.display = 'block';
    }

    // Ensure progress indicator is hidden on initial load
    const progressDiv = document.getElementById('search-progress');
    if (progressDiv) {
      progressDiv.style.display = 'none';
    }

    // Check if URL contains search params and automatically start WebSocket connection
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.toString() != "") {

      // Clear the placeholder and show loading state
      if (cmdOutput) {
        cmdOutput.innerHTML = "<div class='spinner-container'><div class='spinner'></div><div>Loading...</div></div>";
      }
      startWebSocketConnection(urlParams);
    }
  }, []);

  const [showHelp, setShowHelp] = useState(false);
  const [filtersExpanded, setFiltersExpanded] = useState(false);
  const [sampleQueriesExpanded, setSampleQueriesExpanded] = useState(true);

  const handleInputChange = (field, value) => {
    setFormData(prev => ({
      ...prev,
      [field]: value
    }));
  };

  const handleChainChange = (chainId) => {
    handleInputChange('chain_id', chainId || '');

    // Clear previous results when chain changes
    const cmdOutput = document.querySelector('.js-cmd-output');
    if (cmdOutput) {
      cmdOutput.innerHTML = '<div style="color: #888; padding: 20px; text-align: center; font-family: monospace;">Press search to query</div>';
      cmdOutput.style.display = 'block';
    }

    // Hide progress indicator
    const progressDiv = document.getElementById('search-progress');
    if (progressDiv) {
      progressDiv.style.display = 'none';
    }

    // Clear React viewer state if available
    if (window.clearMevlogViewer) {
      window.clearMevlogViewer();
    }

    // Update URL without auto-running query
    const params = new URLSearchParams();
    const updatedFormData = { ...formData, chain_id: chainId || '' };

    Object.entries(updatedFormData).forEach(([key, value]) => {
      if (value !== '' && value !== false) {
        if (typeof value === 'boolean') {
          params.append(key, 'true');
        } else {
          params.append(key, value);
        }
      }
    });

    const queryString = params.toString();
    const url = queryString ? `/search?${queryString}` : '/search';
    window.history.pushState({}, '', url);
  };

  const wsProtocol = () => {
    return window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  };


  const startWebSocketConnection = (params) => {
    const socket = new WebSocket(`${wsProtocol()}//${window.location.host}/ws/search?${params.toString()}`);

    // Show progress indicator
    const progressDiv = document.getElementById('search-progress');
    if (progressDiv) {
      progressDiv.style.display = 'block';
    }

    socket.addEventListener('open', (event) => {
      console.log('Connected to WebSocket server');

      // Always clear React viewer when WebSocket opens to ensure fresh results
      if (window.clearMevlogViewer) {
        window.clearMevlogViewer();
      }
    });

    const cmdOutput = document.querySelector('.js-cmd-output');
    let isFirstMessage = true;

    socket.addEventListener('message', (event) => {
      console.log('Raw message received:', event.data);
      try {
        // Try to parse as JSON first
        const jsonData = JSON.parse(event.data);
        console.log('Parsed JSON data:', jsonData);

        if (window.updateMevlogViewer) {
          console.log('Updating React with JSON data:', jsonData);
          // Send JSON data to React component
          window.updateMevlogViewer(jsonData);
          // Hide the text output when React takes over
          cmdOutput.style.display = 'none';
        } else {
          console.warn('updateMevlogViewer not available, showing in output');
          if (isFirstMessage) {
            cmdOutput.innerHTML = '';
            isFirstMessage = false;
          }
          cmdOutput.insertAdjacentHTML('beforeend', `<pre>${JSON.stringify(jsonData, null, 2)}</pre>`);
        }
      } catch (e) {
        console.error('Error parsing JSON, raw data:', event.data);
        console.error('JSON parse error:', e);
        // If it's not JSON, display as regular text
        if (isFirstMessage) {
          cmdOutput.innerHTML = '';
          isFirstMessage = false;
        }
        cmdOutput.insertAdjacentHTML('beforeend', `<div>${event.data}</div>`);
      }
    });

    socket.addEventListener('close', (event) => {
      console.log('Disconnected from WebSocket server');

      // Hide progress indicator when connection closes
      if (progressDiv) {
        progressDiv.style.display = 'none';
      }
    });

    socket.addEventListener('error', (event) => {
      console.error('WebSocket error:', event);

      // Hide progress indicator on error
      if (progressDiv) {
        progressDiv.style.display = 'none';
      }
    });
  };

  const handleSubmit = (e) => {
    e.preventDefault();

    // Collapse filters when search is clicked
    setFiltersExpanded(false);

    // Clear previous results before starting new search
    const cmdOutput = document.querySelector('.js-cmd-output');
    if (cmdOutput) {
      cmdOutput.innerHTML = '';
      cmdOutput.style.display = 'block'; // Show output container for new results
    }

    // Hide progress indicator from any previous search
    const progressDiv = document.getElementById('search-progress');
    if (progressDiv) {
      progressDiv.style.display = 'none';
    }

    // Clear React viewer state if available
    if (window.clearMevlogViewer) {
      window.clearMevlogViewer();
    }

    // Build query string from form data
    const params = new URLSearchParams();

    Object.entries(formData).forEach(([key, value]) => {
      if (value !== '' && value !== false) {
        if (typeof value === 'boolean') {
          params.append(key, 'true');
        } else {
          params.append(key, value);
        }
      }
    });

    // Update URL without page reload
    const queryString = params.toString();
    const url = queryString ? `/search?${queryString}` : '/search';

    // Update browser URL
    window.history.pushState({}, '', url);

    // Establish WebSocket connection
    startWebSocketConnection(params);
  };

  const toggleFilters = () => {
    setFiltersExpanded(prev => !prev);
  };

  const toggleSampleQueries = () => {
    setSampleQueriesExpanded(prev => !prev);
  };

  // Styling constants
  const formContainerStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '8px',
    padding: '24px',
    marginBottom: '24px',
    fontFamily: 'monospace'
  };

  const sectionStyle = {
    marginBottom: '24px',
    border: '1px solid #333',
    borderRadius: '6px',
    overflow: 'hidden'
  };

  const sectionHeaderStyle = {
    backgroundColor: '#2a2a2a',
    padding: '12px 16px',
    borderBottom: '1px solid #333',
    cursor: 'pointer',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    userSelect: 'none'
  };

  const sectionTitleStyle = {
    color: '#fff',
    fontSize: '14px',
    fontWeight: '600',
    textTransform: 'uppercase',
    letterSpacing: '0.5px'
  };

  const sectionContentStyle = {
    padding: '16px',
    backgroundColor: '#1e1e1e'
  };

  const fieldGridStyle = {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
    gap: '16px'
  };

  const fieldStyle = {
    display: 'flex',
    flexDirection: 'column',
    gap: '6px'
  };

  const labelStyle = {
    color: '#888',
    fontSize: '12px',
    fontWeight: '500',
    textTransform: 'uppercase',
    letterSpacing: '0.5px'
  };

  const inputStyle = {
    backgroundColor: '#2a2a2a',
    border: '1px solid #444',
    borderRadius: '4px',
    color: '#fff',
    fontSize: '14px',
    padding: '8px 12px',
    fontFamily: 'monospace',
    transition: 'border-color 0.2s ease'
  };

  const inputFocusStyle = {
    ...inputStyle,
    borderColor: '#ffd700',
    outline: 'none'
  };

  const checkboxContainerStyle = {
    display: 'flex',
    alignItems: 'center',
    gap: '8px'
  };

  const checkboxStyle = {
    width: '16px',
    height: '16px',
    accentColor: '#ffd700'
  };

  const buttonContainerStyle = {
    display: 'flex',
    gap: '12px',
    alignItems: 'center',
    justifyContent: 'center',
    marginTop: '24px',
    paddingTop: '24px',
    borderTop: '1px solid #333'
  };

  const buttonStyle = {
    backgroundColor: '#ffd700',
    border: '1px solid #ccc',
    borderRadius: '4px',
    color: '#000',
    cursor: 'pointer',
    fontSize: '14px',
    fontWeight: 'bold',
    padding: '12px 24px',
    transition: 'all 0.2s ease',
    fontFamily: 'monospace'
  };

  const helpButtonStyle = {
    backgroundColor: '#2a2a2a',
    border: '1px solid #444',
    borderRadius: '4px',
    color: '#fff',
    cursor: 'pointer',
    fontSize: '14px',
    padding: '12px 24px',
    transition: 'all 0.2s ease',
    fontFamily: 'monospace'
  };

  const helpModalStyle = {
    position: 'fixed',
    top: '0',
    left: '0',
    right: '0',
    bottom: '0',
    backgroundColor: 'rgba(0, 0, 0, 0.8)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex: 1000,
    padding: '20px'
  };

  const helpContentStyle = {
    backgroundColor: '#1a1a1a',
    border: '1px solid #333',
    borderRadius: '8px',
    padding: '24px',
    maxWidth: '800px',
    maxHeight: '80vh',
    overflow: 'auto',
    color: '#fff',
    fontFamily: 'monospace'
  };

  const helpItemStyle = {
    marginBottom: '16px',
    padding: '12px',
    backgroundColor: '#2a2a2a',
    borderRadius: '4px',
    border: '1px solid #333'
  };

  const helpHeaderStyle = {
    color: '#ffd700',
    fontSize: '14px',
    fontWeight: 'bold',
    marginBottom: '8px'
  };

  const helpTextStyle = {
    fontSize: '13px',
    lineHeight: '1.4',
    color: '#ccc'
  };

  const expandIconStyle = {
    color: '#888',
    fontSize: '12px',
    transform: 'rotate(0deg)',
    transition: 'transform 0.2s ease'
  };

  const expandedIconStyle = {
    ...expandIconStyle,
    transform: 'rotate(90deg)'
  };

  const chainSelectorWrapperStyle = {
    marginBottom: '24px'
  };

  const sampleQueryContainerStyle = {
    marginBottom: '24px',
    border: '1px solid #333',
    borderRadius: '6px',
    overflow: 'hidden'
  };

  const sampleQueryStyle = {
    backgroundColor: '#2a2a2a',
    border: 'none',
    borderBottom: '1px solid #333',
    color: '#fff',
    cursor: 'pointer',
    fontSize: '13px',
    fontFamily: 'monospace',
    padding: '12px 16px',
    textAlign: 'left',
    transition: 'background-color 0.2s ease',
    width: '100%'
  };

  const sampleQueries = [
    {
      title: 'Find jaredfromsubway.eth transactions',
      params: {
        from: 'jaredfromsubway.eth',
        chain_id: '1'
      }
    },
    {
      title: 'Query for txs that transferred PEPE token',
      params: {
        event: 'Transfer(address,address,uint256)|0x6982508145454ce325ddbe47a25d4ec3d2311933',
        chain_id: '1'
      }
    },
    {
      title: 'Find transactions that transferred over 100k USDC',
      params: {
        erc20_transfer: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48|ge100gwei',
        chain_id: '1'
      }
    },
    {
      title: 'Find new smart contract deployments',
      params: {
        to: 'CREATE',
        chain_id: '1'
      }
    },
    {
      title: 'Position 0 txs that did not emit any event matching /(Swap).+/ regexp',
      params: {
        position: '0',
        not_event: '/(Swap).+/',
        chain_id: '1'
      }
    },
    {
      title: 'Txs that paid over 0.01 ether in gas fees',
      params: {
        tx_cost: 'ge0.01ether',
        chain_id: '1'
      }
    }
  ].map(query => ({
    ...query,
    params: {
      ...query.params,
      blocks: '100:latest'
    }
  }));

  const applySampleQuery = (sampleParams) => {
    // Reset form data to defaults first
    const resetData = {
      blocks: '',
      position: '',
      from: '',
      to: '',
      event: '',
      not_event: '',
      method: '',
      erc20_transfer: '',
      tx_cost: '',
      gas_price: '',
      chain_id: ''
    };

    // Apply sample parameters
    const updatedData = { ...resetData, ...sampleParams };
    setFormData(updatedData);

    // Expand filters to show the populated fields
    setFiltersExpanded(true);

    // Show placeholder message in results area
    const cmdOutput = document.querySelector('.js-cmd-output');
    if (cmdOutput) {
      cmdOutput.innerHTML = '<div style="color: #888; padding: 20px; text-align: center; font-family: monospace;">Press search to query</div>';
      cmdOutput.style.display = 'block';
    }

    // Clear React viewer state if available
    if (window.clearMevlogViewer) {
      window.clearMevlogViewer();
    }

    // Scroll to filters section after a short delay to ensure DOM updates
    setTimeout(() => {
      // Find the filters section by looking for the element containing "Filters" text
      const filtersHeaders = Array.from(document.querySelectorAll('span')).filter(span =>
        span.textContent === 'Filters'
      );
      if (filtersHeaders.length > 0) {
        const filtersSection = filtersHeaders[0].closest('div[style*="border"]');
        if (filtersSection) {
          // Calculate position with offset for sticky header
          const rect = filtersSection.getBoundingClientRect();
          const offset = 80; // Adjust this value based on your header height
          const targetPosition = window.pageYOffset + rect.top - offset;

          window.scrollTo({
            top: targetPosition,
            behavior: 'smooth'
          });
        }
      }
    }, 100);
  };

  return (
    <div>
      <form onSubmit={handleSubmit}>
        <div style={formContainerStyle}>
          {/* Chain Selector */}
          <div style={chainSelectorWrapperStyle}>
            <ChainSelector
              onChainChange={handleChainChange}
              initialChainId={formData.chain_id ? parseInt(formData.chain_id) : null}
            />
          </div>

          {/* Sample Queries */}
          <div style={sampleQueryContainerStyle}>
            <div
              style={sectionHeaderStyle}
              onClick={toggleSampleQueries}
            >
              <span style={sectionTitleStyle}>Sample Queries [Mainnet]</span>
              <span style={sampleQueriesExpanded ? expandedIconStyle : expandIconStyle}>
                ▶
              </span>
            </div>
            {sampleQueriesExpanded && (
              <div style={sectionContentStyle}>
                {sampleQueries.map((query, index) => (
                  <button
                    key={index}
                    type="button"
                    style={{
                      ...sampleQueryStyle,
                      borderBottom: index === sampleQueries.length - 1 ? 'none' : '1px solid #333'
                    }}
                    onClick={() => applySampleQuery(query.params)}
                    onMouseEnter={(e) => e.target.style.backgroundColor = '#3a3a3a'}
                    onMouseLeave={(e) => e.target.style.backgroundColor = '#2a2a2a'}
                  >
                    {query.title}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Filters Section */}
          <div style={sectionStyle}>
            <div
              style={sectionHeaderStyle}
              onClick={toggleFilters}
            >
              <span style={sectionTitleStyle}>Filters</span>
              <span style={filtersExpanded ? expandedIconStyle : expandIconStyle}>
                ▶
              </span>
            </div>
            {filtersExpanded && (
              <div style={sectionContentStyle}>
                <div style={fieldGridStyle}>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Blocks</label>
                    <input
                      type="text"
                      value={formData.blocks}
                      onChange={(e) => handleInputChange('blocks', e.target.value)}
                      placeholder="e.g. 10:latest"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Position</label>
                    <input
                      type="text"
                      value={formData.position}
                      onChange={(e) => handleInputChange('position', e.target.value)}
                      placeholder="e.g. 0:5"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>From</label>
                    <input
                      type="text"
                      value={formData.from}
                      onChange={(e) => handleInputChange('from', e.target.value)}
                      placeholder="Filter by source"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>To</label>
                    <input
                      type="text"
                      value={formData.to}
                      onChange={(e) => handleInputChange('to', e.target.value)}
                      placeholder="Filter by target"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Event</label>
                    <input
                      type="text"
                      value={formData.event}
                      onChange={(e) => handleInputChange('event', e.target.value)}
                      placeholder="Signature or regexp"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Not Event</label>
                    <input
                      type="text"
                      value={formData.not_event}
                      onChange={(e) => handleInputChange('not_event', e.target.value)}
                      placeholder="Signature or regexp"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Method</label>
                    <input
                      type="text"
                      value={formData.method}
                      onChange={(e) => handleInputChange('method', e.target.value)}
                      placeholder="Filter by method"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>ERC20 Transfer</label>
                    <input
                      type="text"
                      value={formData.erc20_transfer}
                      onChange={(e) => handleInputChange('erc20_transfer', e.target.value)}
                      placeholder="Contract address or address|amount filter"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Tx Cost</label>
                    <input
                      type="text"
                      value={formData.tx_cost}
                      onChange={(e) => handleInputChange('tx_cost', e.target.value)}
                      placeholder="Gas tx cost"
                      style={inputStyle}
                    />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>Gas Price</label>
                    <input
                      type="text"
                      value={formData.gas_price}
                      onChange={(e) => handleInputChange('gas_price', e.target.value)}
                      placeholder="Gas price"
                      style={inputStyle}
                    />
                  </div>
                </div>
              </div>
            )}
          </div>

          <CommandBuilder
            type="search"
            params={formData}
          />

          {/* Action Buttons */}
          <div style={buttonContainerStyle}>
            <button
              type="button"
              onClick={() => setShowHelp(true)}
              style={helpButtonStyle}
            >
              Help
            </button>
            <button
              type="submit"
              style={buttonStyle}
            >
              Search
            </button>
          </div>
        </div>
      </form>

      {/* Help Modal */}
      {showHelp && (
        <div style={helpModalStyle} onClick={() => setShowHelp(false)}>
          <div style={helpContentStyle} onClick={(e) => e.stopPropagation()}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
              <h3 style={{ color: '#ffd700', margin: 0 }}>Search Help</h3>
              <button
                onClick={() => setShowHelp(false)}
                style={{
                  background: 'none',
                  border: 'none',
                  color: '#fff',
                  fontSize: '20px',
                  cursor: 'pointer'
                }}
              >
                ×
              </button>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Blocks:</div>
              <div style={helpTextStyle}>Block number or range to filter by (e.g. <strong>'22030899'</strong>, <strong>'latest'</strong>, <strong>10:latest</strong>, <strong>'22030800:22030900'</strong>)</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Position:</div>
              <div style={helpTextStyle}>Tx position or position range in a block (e.g., <strong>'0'</strong> or <strong>'0:10'</strong>)</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>From:</div>
              <div style={helpTextStyle}>Tx source address or ENS name: (e.g. <strong>jaredfromsubway.eth</strong> or <strong>0xae2fc483527b8ef99eb5d9b44875f005ba1fae13</strong>)</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>To:</div>
              <div style={helpTextStyle}>Tx target address or 'CREATE' for contract creation txs</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Event:</div>
              <div style={helpTextStyle}>Include txs by event names matching the provided regex, signature or source address</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Not Event:</div>
              <div style={helpTextStyle}>Exclude txs by event names matching the provided regex, signature or source address</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Method:</div>
              <div style={helpTextStyle}>Include txs by root method names matching the provided regex, signature or signature hash</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>ERC20 Transfer:</div>
              <div style={helpTextStyle}>Filter transactions with ERC20 Transfer events. Use contract address (e.g. <strong>0xa0b86a33e6ba3bc6c2c5ed1b4b29b5473fd5d2de</strong>) or address with amount filter (e.g. <strong>0xa0b86a33e6ba3bc6c2c5ed1b4b29b5473fd5d2de|ge1000</strong> for transfers ≥ 1000 tokens)</div>
            </div>

            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Tx Cost:</div>
              <div style={helpTextStyle}>Filter by tx cost (e.g. <strong>ge0.01ether</strong> or <strong>le0.005ether</strong>)</div>
            </div>


            <div style={helpItemStyle}>
              <div style={helpHeaderStyle}>Gas Price:</div>
              <div style={helpTextStyle}>Filter by effective gas price (e.g. <strong>ge5gwei</strong> or <strong>le2gwei</strong>)</div>
            </div>



          </div>
        </div>
      )}

    </div>
  );
};

export default SearchForm;