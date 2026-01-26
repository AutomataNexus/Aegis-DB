# Aegis DB Installation

## Quick Start

1. Extract the archive:
   ```bash
   tar -xzf aegis-db-0.2.0-linux-x86_64.tar.gz
   cd aegis-db-0.2.0-linux-x86_64
   ```

2. Set admin credentials:
   ```bash
   export AEGIS_ADMIN_USERNAME=admin
   export AEGIS_ADMIN_PASSWORD='your-secure-password'
   ```

3. Start the server:
   ```bash
   ./start.sh
   ```

4. Serve the dashboard (optional):
   ```bash
   # Using Python
   cd dashboard && python3 -m http.server 8000

   # Or using Node.js
   npx serve dashboard -p 8000
   ```

5. Access the dashboard at http://localhost:8000

## Configuration

Edit `config/aegis.toml` or use environment variables:

- `AEGIS_HOST` - Server bind address (default: 127.0.0.1)
- `AEGIS_PORT` - Server port (default: 9090)
- `AEGIS_DATA_DIR` - Data directory (default: ./data)
- `AEGIS_ADMIN_USERNAME` - Initial admin username
- `AEGIS_ADMIN_PASSWORD` - Initial admin password
- `AEGIS_CORS_ORIGINS` - Comma-separated CORS origins

## CLI Usage

```bash
# Connect to local server
./bin/aegis --host localhost --port 9090

# Execute a query
./bin/aegis query "SELECT * FROM users"

# Interactive mode
./bin/aegis shell
```

## Documentation

See the `docs/` folder for:
- USER_GUIDE.md - Full usage documentation
- SECURITY.md - Security configuration
- COMPLIANCE.md - GDPR/HIPAA compliance features

## Support

- GitHub: https://github.com/AutomataNexus/Aegis-DB
- Email: Devops@automatanexus.com
- Commercial licensing: See COMMERCIAL.md
