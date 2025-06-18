# Schema Collection Script (`collect_schema.sh`)

This script is used to download official Model Context Protocol (MCP) JSON schemas from the main protocol repository and save them locally within the `schemas/` directory of this project.

## Purpose

-   **Local Schema Access:** Provides local copies of specific MCP schema versions for development, testing, and reference.
-   **Version Management:** Allows fetching schemas for particular protocol versions.
-   **Repository Commits:** Downloaded schemas are intended to be committed to this repository to ensure consistent schema versions are available for all developers and CI processes.

## Usage

The script should be run from the root of the repository.

### Download Schema for Latest Supported Version

To download the schema corresponding to the `LATEST_PROTOCOL_VERSION` defined in `src/types.rs`:

```bash
./scripts/collect_schema.sh
```

This will save the schema to `schemas/{VERSION}/schema.json`, where `{VERSION}` is the value of `LATEST_PROTOCOL_VERSION`.

### Download Schema for a Specific Version

To download a schema for a specific protocol version, provide the version string as an argument:

```bash
./scripts/collect_schema.sh <VERSION>
```

For example:

```bash
./scripts/collect_schema.sh 2024-01-01
```

This will download the schema for version `2024-01-01` and save it to `schemas/2024-01-01/schema.json`.

## Script Details

-   The script uses `curl` to download the schema.
-   It constructs the download URL based on the pattern: `https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/{VERSION}/schema.json`.
-   It creates the necessary versioned subdirectories within `schemas/` automatically.
-   Error messages will be printed if the download fails or if the `LATEST_PROTOCOL_VERSION` cannot be determined from the source code.

## Schema Usage in Tests

When running project tests with the `schema-validation` feature enabled (e.g., via `cargo test --features schema-validation`), the schema validation logic is designed to be both robust and developer-friendly:

1.  **Attempt Local Schema Load:**
    The system will first attempt to load the required schema from a local file. Specifically, it looks for the schema corresponding to the `LATEST_PROTOCOL_VERSION` (defined in `src/types.rs`) at the path `schemas/{LATEST_PROTOCOL_VERSION}/schema.json` (relative to the crate root). If found and valid, this local schema is used, providing fast and offline-capable tests.

2.  **Network Fallback:**
    If the local schema file is not found, cannot be read, or fails to parse as valid JSON:
    *   A warning message will be logged detailing the issue with the local file.
    *   The system will then automatically fall back to fetching the schema from its official remote URL (on `raw.githubusercontent.com`).
    *   Tests will proceed using this network-fetched schema.

**Recommendation:**
While the network fallback provides convenience, it's still **highly recommended to use the `./scripts/collect_schema.sh` script** to download and commit the schema(s) you are working against. Using local schemas ensures:
-   **Faster tests:** Avoids network latency.
-   **Reliable tests:** Not dependent on network connectivity or the remote server's availability.
-   **Offline capability:** Allows running schema validation tests without an internet connection.
-   **Consistent schema:** Guarantees tests run against a known, version-controlled schema.
