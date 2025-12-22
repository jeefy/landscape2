# Landscape2 MCP Server

A Model Context Protocol (MCP) server for the CNCF Landscape.

## Features

- **Query Landscape**: Filter and search for items in the landscape.
- **Finance Data**: Retrieve funding and acquisition data for organizations.

## Building

### Local Build

```bash
cargo build -p landscape2-mcp-server
```

### Docker Build

From the workspace root:

```bash
docker build -f crates/mcp-server/Dockerfile -t landscape2-mcp-server .
```

## Usage

### Running Locally

```bash
./target/debug/landscape2-mcp-server
```

### Running with Docker

```bash
docker run -i landscape2-mcp-server
```

## Tools

### `query_landscape`

Queries the landscape for items matching specific criteria.

- `name`: (Optional) Filter by name (case-insensitive).
- `category`: (Optional) Filter by category.
- `maturity`: (Optional) Filter by maturity (e.g., "graduated", "incubating").

### `get_finance_data`

Retrieves finance data (funding, acquisitions) for organizations.

- `min_funding`: (Optional) Minimum funding amount in USD.
- `limit`: (Optional) Number of results to return (default: 10).
- `page`: (Optional) Page number for pagination.
