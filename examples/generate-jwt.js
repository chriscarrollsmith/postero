#!/usr/bin/env node

// JWT token generator for PostgREST API testing
// Install dependencies: npm install jsonwebtoken
// Usage: node generate-jwt.js [group_id] [role]

const jwt = require('jsonwebtoken');

// This should match the JWT secret in docker-compose.yml
const JWT_SECRET = 'your-very-secure-and-long-jwt-secret-that-is-at-least-32-chars';

// Get command line arguments
const groupId = process.argv[2] || '123456';
const role = process.argv[3] || 'api_user';

// Token payload
const payload = {
  role: role,
  group_id: parseInt(groupId),
  library_id: parseInt(groupId), // Alternative claim name
  exp: Math.floor(Date.now() / 1000) + (60 * 60 * 24) // Expires in 24 hours
};

// Generate token
const token = jwt.sign(payload, JWT_SECRET);

console.log('🔐 JWT Token Generated');
console.log('====================');
console.log(`Group ID: ${groupId}`);
console.log(`Role: ${role}`);
console.log(`Expires: ${new Date(payload.exp * 1000).toISOString()}`);
console.log('');
console.log('Token:');
console.log(token);
console.log('');
console.log('📋 Usage Examples:');
console.log('');
console.log('# Get items for your group:');
console.log(`curl -H "Authorization: Bearer ${token}" \\`);
console.log(`     "http://localhost:3000/items_view"`);
console.log('');
console.log('# Get collections for your group:');
console.log(`curl -H "Authorization: Bearer ${token}" \\`);
console.log(`     "http://localhost:3000/collections_view"`);
console.log('');
console.log('# Create a new item (POST):');
console.log(`curl -X POST -H "Authorization: Bearer ${token}" \\`);
console.log(`     -H "Content-Type: application/json" \\`);
console.log(`     -d '{"key":"TESTKEY1","library":${groupId},"data":{"itemType":"note","note":"Test note"}}' \\`);
console.log(`     "http://localhost:3000/items"`);
console.log('');
console.log('💡 Notes:');
console.log('- Row-Level Security will automatically filter results by your group_id');
console.log('- Use role "api_anon" for read-only access');
console.log('- Use role "api_user" for full CRUD access');
console.log('- Make sure the group_id matches your Zotero library ID'); 