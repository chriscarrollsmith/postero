-- Enable required PostgreSQL extensions

-- Enable pg_trgm extension for trigram indexes and text search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Enable uuid-ossp extension for UUID generation (if needed)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Enable pgcrypto extension for cryptographic functions (if needed for JWT)
CREATE EXTENSION IF NOT EXISTS pgcrypto; 