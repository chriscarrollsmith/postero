#!/bin/bash

# Test script for PostgREST API endpoints
# Make sure PostgreSQL and PostgREST are running: docker compose up -d

BASE_URL="http://localhost:3000"

echo "🧪 Testing PostgREST API Endpoints"
echo "=================================="

# Test 1: Check if PostgREST is running and get OpenAPI spec
echo "📋 1. Checking PostgREST OpenAPI specification..."
curl -s "$BASE_URL/" | jq '.info.title' 2>/dev/null || echo "❌ PostgREST not responding or jq not installed"

# Test 2: List all available endpoints
echo ""
echo "📋 2. Available endpoints:"
curl -s "$BASE_URL/" | jq '.paths | keys[]' 2>/dev/null || echo "❌ Could not retrieve endpoints"

# Test 3: Test groups_view
echo ""
echo "📋 3. Testing groups_view..."
curl -s "$BASE_URL/groups_view" | jq '. | length' 2>/dev/null || echo "❌ No groups found or error"

# Test 4: Test items_view (should be empty without data)
echo ""
echo "📋 4. Testing items_view..."
curl -s "$BASE_URL/items_view" | jq '. | length' 2>/dev/null || echo "❌ Items view error"

# Test 5: Test collections_view
echo ""
echo "📋 5. Testing collections_view..."
curl -s "$BASE_URL/collections_view" | jq '. | length' 2>/dev/null || echo "❌ Collections view error"

# Test 6: Test materialized views
echo ""
echo "📋 6. Testing collection_name_hier materialized view..."
curl -s "$BASE_URL/collection_name_hier" | jq '. | length' 2>/dev/null || echo "❌ Collection hierarchy view error"

echo ""
echo "📋 7. Testing item_type_hier materialized view..."
curl -s "$BASE_URL/item_type_hier" | jq '. | length' 2>/dev/null || echo "❌ Item type hierarchy view error"

# Test 7: Test RPC function
echo ""
echo "📋 8. Testing RPC function get_collection_by_name..."
curl -s -X POST "$BASE_URL/rpc/get_collection_by_name" \
     -H "Content-Type: application/json" \
     -d '{"p_group_id": 123456, "p_name": "Test Collection"}' | \
     jq '. | length' 2>/dev/null || echo "❌ RPC function error (expected if no data)"

# Test 8: Test filtering capabilities
echo ""
echo "📋 9. Testing filtering (should return empty until data is synced)..."
curl -s "$BASE_URL/items_view?item_type=eq.journalArticle" | jq '. | length' 2>/dev/null || echo "❌ Filtering error"

# Test 9: Test ordering
echo ""
echo "📋 10. Testing ordering..."
curl -s "$BASE_URL/items_view?order=modified.desc&limit=5" | jq '. | length' 2>/dev/null || echo "❌ Ordering error"

echo ""
echo "✅ PostgREST API test completed!"
echo ""
echo "💡 Tips:"
echo "   - If tests show errors, check: docker logs zotero_postgrest"
echo "   - If no data, run sync first: go run cmd/sync/*.go -c zoterosync.toml"
echo "   - For JWT testing, use examples/generate-jwt.js"
echo "   - Full API docs: curl $BASE_URL/ | jq ." 