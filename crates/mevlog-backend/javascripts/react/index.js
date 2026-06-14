import React from 'react';
import ReactDOM from 'react-dom/client';
import MevlogViewer from './MevlogViewer';
import ExploreViewer from './ExploreViewer';
import ChainSelector from './ChainSelector';
import SearchForm from './SearchForm';
import CommandBuilder from './CommandBuilder';
import PRESETS from './presets';

// Shared with the non-bundled hero script (scripts.js) via this global.
window.MEVLOG_PRESETS = PRESETS;

// Export for global usage
window.MevlogReact = {
  MevlogViewer,
  ExploreViewer,
  ChainSelector,
  SearchForm,
  CommandBuilder,
  React,
  ReactDOM
};

// Auto-mount if container exists
document.addEventListener('DOMContentLoaded', () => {
  const container = document.getElementById('mevlog-react-root');
  if (container) {
    const root = ReactDOM.createRoot(container);
    root.render(React.createElement(MevlogViewer));
    window.mevlogReactRoot = root;
  }
  
  const exploreContainer = document.getElementById('explore-react-root');
  if (exploreContainer) {
    const root = ReactDOM.createRoot(exploreContainer);
    root.render(React.createElement(ExploreViewer));
    window.exploreReactRoot = root;
  }
  
  const chainSelectorContainer = document.getElementById('chain-selector-react-root');
  if (chainSelectorContainer) {
    const chainIdInput = document.getElementById('chain_id');
    const initialChainId = chainIdInput && chainIdInput.value ? parseInt(chainIdInput.value) : null;
    
    const handleChainChange = (chainId) => {
      if (chainIdInput) {
        chainIdInput.value = chainId || '';
      }
    };
    
    const root = ReactDOM.createRoot(chainSelectorContainer);
    root.render(React.createElement(ChainSelector, {
      onChainChange: handleChainChange,
      initialChainId: initialChainId
    }));
    window.chainSelectorReactRoot = root;
  }
  
  const searchFormContainer = document.getElementById('search-form-react-root');
  if (searchFormContainer) {
    // Extract initial values from data attributes
    const initialValues = {
      sql: searchFormContainer.dataset.sql || '',
      chain_id: searchFormContainer.dataset.chainId || ''
    };
    
    const root = ReactDOM.createRoot(searchFormContainer);
    root.render(React.createElement(SearchForm, {
      initialValues: initialValues
    }));
    window.searchFormReactRoot = root;
  }
  
});