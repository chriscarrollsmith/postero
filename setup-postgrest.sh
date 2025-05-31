#!/bin/bash

# Setup script for ZoteroSync with PostgREST
# This script sets up the entire PostgREST infrastructure

set -e  # Exit on any error

echo "🚀 Setting up ZoteroSync with PostgREST"
echo "======================================"

# Check if Docker is installed and running
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed. Please install Docker first."
    exit 1
fi

if ! docker info &> /dev/null; then
    echo "❌ Docker is not running. Please start Docker first."
    exit 1
fi

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo "❌ docker-compose is not available. Please install docker-compose."
    exit 1
fi

# Function to run docker-compose (handle both docker-compose and docker compose)
run_docker_compose() {
    if command -v docker-compose &> /dev/null; then
        docker-compose "$@"
    else
        docker compose "$@"
    fi
}

echo ""
echo "📦 1. Starting Docker services..."
run_docker_compose up -d

echo ""
echo "⏳ 2. Waiting for PostgreSQL to be ready..."
until run_docker_compose exec postgres pg_isready -U postgres &> /dev/null; do
    echo "   Waiting for PostgreSQL..."
    sleep 2
done

echo ""
echo "🔧 3. Running database initialization scripts..."

# Run all initialization scripts in order
scripts=(
    "00-create-extensions.sql"
    "01-create-enums.sql"
    "02-create-tables.sql"
    "03-create-materialized-views.sql"
    "04-create-api-roles.sql"
    "05-create-api-views.sql"
    "06-row-level-security.sql"
)

for script in "${scripts[@]}"; do
    echo "   📄 Running init-scripts/$script..."
    if run_docker_compose exec -T postgres psql -U postgres -d zotero < "init-scripts/$script"; then
        echo "   ✅ $script completed successfully"
    else
        echo "   ❌ $script failed"
        exit 1
    fi
done

echo ""
echo "🔍 4. Verifying database setup..."

# Check tables
echo "   📊 Checking tables..."
table_count=$(run_docker_compose exec -T postgres psql -U postgres -d zotero -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE';" | tr -d ' \n')
echo "   Found $table_count tables"

# Check views
echo "   👁️  Checking views..."
view_count=$(run_docker_compose exec -T postgres psql -U postgres -d zotero -t -c "SELECT COUNT(*) FROM information_schema.views WHERE table_schema = 'public';" | tr -d ' \n')
echo "   Found $view_count views"

# Check materialized views
echo "   🏗️  Checking materialized views..."
matview_count=$(run_docker_compose exec -T postgres psql -U postgres -d zotero -t -c "SELECT COUNT(*) FROM pg_matviews WHERE schemaname = 'public';" | tr -d ' \n')
echo "   Found $matview_count materialized views"

# Check roles
echo "   👤 Checking API roles..."
role_count=$(run_docker_compose exec -T postgres psql -U postgres -d zotero -t -c "SELECT COUNT(*) FROM pg_roles WHERE rolname IN ('api_anon', 'api_user');" | tr -d ' \n')
echo "   Found $role_count API roles"

echo ""
echo "🌐 5. Testing PostgREST API..."

# Wait for PostgREST to be ready
max_attempts=30
attempt=1
while ! curl -s http://localhost:3000/ > /dev/null; do
    if [ $attempt -ge $max_attempts ]; then
        echo "   ❌ PostgREST did not start within expected time"
        echo "   Check logs: docker logs zotero_postgrest"
        exit 1
    fi
    echo "   Waiting for PostgREST... (attempt $attempt/$max_attempts)"
    sleep 2
    ((attempt++))
done

echo "   ✅ PostgREST is responding"

# Test API endpoints
echo "   🧪 Testing API endpoints..."
if curl -s "http://localhost:3000/items_view" > /dev/null; then
    echo "   ✅ items_view endpoint working"
else
    echo "   ❌ items_view endpoint failed"
fi

if curl -s "http://localhost:3000/collections_view" > /dev/null; then
    echo "   ✅ collections_view endpoint working"
else
    echo "   ❌ collections_view endpoint failed"
fi

echo ""
echo "🎉 Setup completed successfully!"
echo ""
echo "📋 What's available:"
echo "   🐘 PostgreSQL: localhost:5432 (postgres/postgres)"
echo "   🌐 PostgREST API: http://localhost:3000"
echo "   📦 MinIO S3: http://localhost:9000 (minioadmin/minioadmin)"
echo "   🖥️  MinIO Console: http://localhost:9001"
echo ""
echo "📖 Next steps:"
echo "   1. Configure your Zotero API key in zoterosync.toml"
echo "   2. Run your first sync: go run cmd/sync/*.go -c zoterosync.toml"
echo "   3. Test the API: ./examples/test-postgrest-api.sh"
echo "   4. Generate JWT tokens: cd examples && npm install && node generate-jwt.js YOUR_GROUP_ID"
echo ""
echo "📚 API Documentation:"
echo "   OpenAPI spec: curl http://localhost:3000/"
echo "   View online: open http://localhost:3000/"
echo ""
echo "🛠️  Troubleshooting:"
echo "   Check PostgreSQL logs: docker logs zotero_postgres"
echo "   Check PostgREST logs: docker logs zotero_postgrest"
echo "   Check all services: docker ps" 