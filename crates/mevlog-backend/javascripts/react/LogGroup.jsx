import React from 'react';
import Log from './Log';

const LogGroup = ({ logGroup }) => {
  return (
    <div className="log-group">
      <div className="log-group-header">
        <span className="source">Contract: {logGroup.source}</span>
      </div>
      <div className="logs">
        {logGroup.logs.map((log, idx) => (
          <Log key={idx} log={log} />
        ))}
      </div>
    </div>
  );
};

export default LogGroup;