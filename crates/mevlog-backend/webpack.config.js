const path = require('path');

module.exports = {
  entry: './javascripts/react/index.js',
  output: {
    path: path.resolve(__dirname, 'javascripts/dist'),
    filename: 'react-bundle.js',
    library: 'MevlogReact',
    libraryTarget: 'umd',
  },
  module: {
    rules: [
      {
        test: /\.(js|jsx)$/,
        exclude: /node_modules/,
        use: {
          loader: 'babel-loader',
          options: {
            presets: ['@babel/preset-env', '@babel/preset-react']
          }
        }
      }
    ]
  },
  resolve: {
    extensions: ['.js', '.jsx']
  },
  externals: {
    react: 'React',
    'react-dom': 'ReactDOM'
  }
};