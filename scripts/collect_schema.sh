#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
MCP_SDK_TYPES_FILE="src/types.rs" # Corrected path
SCHEMA_BASE_URL="https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema"
SCHEMAS_OUTPUT_ROOT_DIR="schemas"

# --- Helper Functions ---
extract_latest_protocol_version() {
    # Extracts LATEST_PROTOCOL_VERSION from mcp_sdk/src/types.rs
    # Example line: pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";
    local version_line
    version_line=$(grep 'LATEST_PROTOCOL_VERSION: &str =' "${MCP_SDK_TYPES_FILE}")

    if [[ -z "${version_line}" ]]; then
        echo "Error: Could not find LATEST_PROTOCOL_VERSION in ${MCP_SDK_TYPES_FILE}" >&2
        exit 1
    fi

    # Extracts the version string using awk, getting the second quoted string
    # awk -F'"' -> sets " as the field separator
    # '{print $2}' -> prints the second field (the version string)
    awk -F'"' '{print $2}' <<< "${version_line}"
}

# --- Main Script Logic ---

# Determine the version to use
DEFAULT_VERSION=$(extract_latest_protocol_version)
VERSION="${1:-${DEFAULT_VERSION}}" # Use $1 if provided, else use default

if [[ -z "${VERSION}" ]]; then
    echo "Error: Version could not be determined." >&2
    exit 1
fi

echo "Targeting schema version: ${VERSION}"

# Construct URL and output paths
FULL_URL="${SCHEMA_BASE_URL}/${VERSION}/schema.json"
OUTPUT_DIR="${SCHEMAS_OUTPUT_ROOT_DIR}/${VERSION}"
OUTPUT_FILE="${OUTPUT_DIR}/schema.json"

# Create the output directory
echo "Creating output directory: ${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"

# Download the schema
echo "Downloading schema from: ${FULL_URL}"
# curl options:
# -L: follow redirects
# -f: fail silently on server errors (no HTML error page output), return non-zero exit code
# -s: silent mode (don't show progress meter)
# -o: output file
if curl -L -f -s -o "${OUTPUT_FILE}" "${FULL_URL}"; then
    echo "Schema for version ${VERSION} downloaded successfully to ${OUTPUT_FILE}"
else
    echo "Error: Failed to download schema for version ${VERSION} from ${FULL_URL}" >&2
    # Optional: remove partially downloaded file or empty directory if curl failed
    # rm -f "${OUTPUT_FILE}"
    # if [ -d "${OUTPUT_DIR}" ] && [ -z "$(ls -A "${OUTPUT_DIR}")" ]; then rmdir "${OUTPUT_DIR}"; fi
    exit 1
fi

exit 0
