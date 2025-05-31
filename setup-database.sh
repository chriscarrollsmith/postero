#!/bin/bash

# ZoteroSync Database Setup Script (Legacy)
# For the new PostgREST setup, use: ./setup-postgrest.sh
# This script sets up only the basic PostgreSQL database for ZoteroSync

set -e  # Exit on any error

echo "⚠️  NOTICE: This is the legacy database setup script."
echo "   For the new PostgREST API setup, use: ./setup-postgrest.sh"
echo ""
echo "🚀 Setting up ZoteroSync PostgreSQL database (basic setup)..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker and try again."
    exit 1
fi

# Start PostgreSQL container
echo "📦 Starting PostgreSQL container..."
docker compose up -d

# Wait for PostgreSQL to be ready
echo "⏳ Waiting for PostgreSQL to be ready..."
sleep 10

# Function to run SQL commands
run_sql() {
    docker exec -i zotero_postgres psql -U postgres -d zotero "$@"
}

# Check if PostgreSQL is ready
echo "🔍 Checking PostgreSQL connection..."
if ! run_sql -c "SELECT 1;" > /dev/null 2>&1; then
    echo "❌ PostgreSQL is not ready. Please check the container logs:"
    echo "   docker logs zotero_postgres"
    exit 1
fi

echo "✅ PostgreSQL is ready!"

# Run initialization scripts
echo "📋 Creating database schema..."

echo "  → Creating enum types..."
run_sql < init-scripts/01-create-enums.sql 2>/dev/null || echo "    (Enums already exist)"

echo "  → Creating tables and indexes..."
run_sql < init-scripts/02-create-tables.sql 2>/dev/null || echo "    (Tables already exist)"

echo "  → Creating materialized views..."
run_sql < init-scripts/03-create-materialized-views.sql 2>/dev/null || echo "    (Views already exist)"

# Enable extensions
echo "  → Enabling PostgreSQL extensions..."
run_sql -c "CREATE EXTENSION IF NOT EXISTS pg_trgm;" > /dev/null

# Refresh materialized views
echo "  → Refreshing materialized views..."
run_sql -c "REFRESH MATERIALIZED VIEW CONCURRENTLY public.collection_name_hier;" > /dev/null
run_sql -c "REFRESH MATERIALIZED VIEW CONCURRENTLY public.item_type_hier;" > /dev/null

# Verify setup
echo "🔍 Verifying database setup..."
TABLES=$(run_sql -c "\dt" | grep -c "public |" || true)
VIEWS=$(run_sql -c "\dm" | grep -c "public |" || true)
TYPES=$(run_sql -c "\dT" | grep -c "public |" || true)

echo "✅ Database setup complete!"
echo "📊 Summary:"
echo "   • Tables: $TABLES"
echo "   • Materialized Views: $VIEWS" 
echo "   • Custom Types: $TYPES"
echo ""
echo "🎯 Next steps:"
echo "   1. Copy configs/zoterosync.toml-template to zoterosync.toml"
echo "   2. Add your Zotero API key to the config file"
echo "   3. Run: go run cmd/sync/main.go -c zoterosync.toml"
echo ""
echo "💡 Your personal library will use your Zotero user ID."
echo "   Find your user ID at: https://www.zotero.org/settings/keys" 