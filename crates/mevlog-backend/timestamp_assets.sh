#!/bin/bash
set -euo pipefail 

REMOTE_ENV_FILE=".env-remote"
LOCAL_ENV_FILE=".envrc"
CURRENT_TIMESTAMP=$(date +%s)
sed -i '' "s/^export DEPLOYED_AT=.*/export DEPLOYED_AT=$CURRENT_TIMESTAMP/" "$REMOTE_ENV_FILE"
sed -i '' "s/^export DEPLOYED_AT=.*/export DEPLOYED_AT=$CURRENT_TIMESTAMP/" "$LOCAL_ENV_FILE"
direnv allow

ASSETS_FOLDER="assets"
SCRIPTS_FOLDER="javascripts"
STYLES_FOLDER="styles"
SCRIPTS_SUFFIX="scripts.js"
STYLES_SUFFIX="styles.css"
TERMINAL_SUFFIX="terminal.css"


copy_file() {
    local source_folder=$1
    local source_file=$2
    local target_suffix=$3

    if [[ -f "$source_folder/$source_file" ]]; then
        local new_file="$ASSETS_FOLDER/${CURRENT_TIMESTAMP}-$target_suffix"
        cp "$source_folder/$source_file" "$new_file"
        echo "Copied $source_folder/$source_file to $new_file"
    else
        echo "Source file $source_folder/$source_file not found"
    fi
}

# Build React components if Node.js is available
if command -v npm &> /dev/null; then
    echo "Building React components..."
    npm install --silent
    npm run build
    echo "React build completed"
else
    echo "npm not found, skipping React build"
fi

rm -f "$ASSETS_FOLDER"/*
echo "Assets folder cleaned"
copy_file "$SCRIPTS_FOLDER" "scripts.js" "$SCRIPTS_SUFFIX"
copy_file "$STYLES_FOLDER" "styles.css" "$STYLES_SUFFIX"
copy_file "$STYLES_FOLDER" "terminal.css" "$TERMINAL_SUFFIX"

# Copy React bundle if it exists
if [[ -f "$SCRIPTS_FOLDER/dist/react-bundle.js" ]]; then
    cp "$SCRIPTS_FOLDER/dist/react-bundle.js" "$ASSETS_FOLDER/${CURRENT_TIMESTAMP}-react-bundle.js"
    echo "Copied React bundle to $ASSETS_FOLDER/${CURRENT_TIMESTAMP}-react-bundle.js"
fi

cp media/* "$ASSETS_FOLDER"
