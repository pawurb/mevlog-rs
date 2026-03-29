import React from 'react';

const Log = ({ log }) => {
  return (
    <div className="log-entry">
      <div className="log-signature">{log.signature}</div>
      {log.topics && log.topics.length > 0 && (
        <div className="log-topics">
          {log.topics.map((topic, idx) => (
            <span key={idx} className="topic">{topic}</span>
          ))}
        </div>
      )}
      {log.data && (
        <div className="log-data">{log.data}</div>
      )}
    </div>
  );
};

export default Log;