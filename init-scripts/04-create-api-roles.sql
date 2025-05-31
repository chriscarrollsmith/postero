-- Create API roles for PostgREST authentication and authorization

-- Create roles for API access
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'api_anon') THEN
        CREATE ROLE api_anon NOLOGIN;
    END IF;
END$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'api_user') THEN
        CREATE ROLE api_user NOLOGIN;
    END IF;
END$$;

-- Grant basic connection privileges
GRANT CONNECT ON DATABASE zotero TO api_anon;
GRANT CONNECT ON DATABASE zotero TO api_user;

-- Grant schema usage
GRANT USAGE ON SCHEMA public TO api_anon;
GRANT USAGE ON SCHEMA public TO api_user;

-- Grant sequence usage for inserts
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO api_user;

-- Grant permissions on tables for api_anon (read-only access to safe views)
GRANT SELECT ON public.collection_name_hier TO api_anon;
GRANT SELECT ON public.item_type_hier TO api_anon;

-- Grant permissions on tables for api_user (full CRUD access)
GRANT SELECT, INSERT, UPDATE, DELETE ON public.items TO api_user;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.collections TO api_user;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.tags TO api_user;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.groups TO api_user;
GRANT SELECT ON public.syncgroups TO api_user;

-- Grant access to materialized views
GRANT SELECT ON public.collection_name_hier TO api_user;
GRANT SELECT ON public.item_type_hier TO api_user;

-- Allow api_user to refresh materialized views
GRANT SELECT, DELETE, TRIGGER ON public.collection_name_hier TO api_user;
GRANT SELECT, DELETE, TRIGGER ON public.item_type_hier TO api_user;

-- Create a function to get current user's group access
-- This will be used for row-level security later
CREATE OR REPLACE FUNCTION public.get_user_group_id()
RETURNS bigint AS $$
BEGIN
    -- Extract group_id from JWT claims
    -- This assumes the JWT contains a group_id claim
    RETURN COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint,
        NULL
    );
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Grant execute on utility functions
GRANT EXECUTE ON FUNCTION public.get_user_group_id() TO api_user;
GRANT EXECUTE ON FUNCTION public.get_user_group_id() TO api_anon;

-- Add comments for API documentation
COMMENT ON TABLE public.items IS 'Stores Zotero items including metadata, attachments, and notes';
COMMENT ON COLUMN public.items.key IS 'Unique 8-character Zotero item key';
COMMENT ON COLUMN public.items.library IS 'Library/group ID this item belongs to';
COMMENT ON COLUMN public.items.data IS 'JSON data containing item metadata (title, creators, etc.)';
COMMENT ON COLUMN public.items.meta IS 'JSON metadata about the item sync status and processing';

COMMENT ON TABLE public.collections IS 'Stores Zotero collections and their hierarchical relationships';
COMMENT ON COLUMN public.collections.key IS 'Unique 8-character Zotero collection key';
COMMENT ON COLUMN public.collections.library IS 'Library/group ID this collection belongs to';
COMMENT ON COLUMN public.collections.data IS 'JSON data containing collection metadata (name, parent, etc.)';

COMMENT ON TABLE public.tags IS 'Stores tags associated with items in Zotero libraries';
COMMENT ON COLUMN public.tags.tag IS 'The tag name/text';
COMMENT ON COLUMN public.tags.library IS 'Library/group ID this tag belongs to';

COMMENT ON MATERIALIZED VIEW public.collection_name_hier IS 'Hierarchical view of collections showing parent-child relationships and full paths';
COMMENT ON MATERIALIZED VIEW public.item_type_hier IS 'Hierarchical view of items showing parent-child relationships and item types'; 