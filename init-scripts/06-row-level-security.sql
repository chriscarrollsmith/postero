-- Implement Row-Level Security for multi-tenant data isolation

-- Enable RLS on main tables
ALTER TABLE public.items ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.tags ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.libraries ENABLE ROW LEVEL SECURITY;

-- Create RLS policies for items table
-- Policy: Users can only access items belonging to their library_id and library_type
DROP POLICY IF EXISTS items_library_isolation ON public.items;
CREATE POLICY items_library_isolation
ON public.items
FOR ALL
TO api_user
USING (
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Allow anonymous users to see items if library_id and library_type are explicitly provided and permitted
DROP POLICY IF EXISTS items_anon_access ON public.items;
CREATE POLICY items_anon_access
ON public.items
FOR SELECT
TO api_anon
USING (
    deleted = false AND
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Create RLS policies for collections table
DROP POLICY IF EXISTS collections_library_isolation ON public.collections;
CREATE POLICY collections_library_isolation
ON public.collections
FOR ALL
TO api_user
USING (
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

DROP POLICY IF EXISTS collections_anon_access ON public.collections;
CREATE POLICY collections_anon_access
ON public.collections
FOR SELECT
TO api_anon
USING (
    deleted = false AND
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Create RLS policies for tags table
DROP POLICY IF EXISTS tags_library_isolation ON public.tags;
CREATE POLICY tags_library_isolation
ON public.tags
FOR ALL
TO api_user
USING (
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

DROP POLICY IF EXISTS tags_anon_access ON public.tags;
CREATE POLICY tags_anon_access
ON public.tags
FOR SELECT
TO api_anon
USING (
    library_id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Create RLS policies for libraries table
DROP POLICY IF EXISTS libraries_access ON public.libraries;
CREATE POLICY libraries_access
ON public.libraries
FOR SELECT
TO api_user, api_anon
USING (
    id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Allow api_user to modify their own library
DROP POLICY IF EXISTS libraries_modify ON public.libraries;
CREATE POLICY libraries_modify
ON public.libraries
FOR UPDATE
TO api_user
USING (
    id = COALESCE(
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_library_id', true)::bigint
    ) AND
    library_type = COALESCE(
        current_setting('request.jwt.claims.library_type', true)::public.library_type,
        current_setting('app.current_library_type', true)::public.library_type
    )
);

-- Create secure views that include RLS
-- These views will automatically filter by library_id and library_type when RLS is enabled
CREATE OR REPLACE VIEW public.secure_items_view AS
SELECT * FROM public.items_view;

CREATE OR REPLACE VIEW public.secure_collections_view AS
SELECT * FROM public.collections_view;

CREATE OR REPLACE VIEW public.secure_tags_view AS
SELECT * FROM public.tags_view;

CREATE OR REPLACE VIEW public.secure_libraries_view AS
SELECT * FROM public.libraries_view;

-- Grant permissions on secure views
GRANT SELECT ON public.secure_items_view TO api_anon, api_user;
GRANT SELECT ON public.secure_collections_view TO api_anon, api_user;
GRANT SELECT ON public.secure_tags_view TO api_anon, api_user;
GRANT SELECT ON public.secure_libraries_view TO api_anon, api_user;

-- Function to set library context for API calls
CREATE OR REPLACE FUNCTION public.set_library_context(p_library_id bigint, p_library_type public.library_type)
RETURNS void AS $$
BEGIN
    -- This function can be called by applications to set the library context
    -- It will be used by RLS policies
    PERFORM set_config('app.current_library_id', p_library_id::text, true);
    PERFORM set_config('app.current_library_type', p_library_type::text, true);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Grant execute on context function
GRANT EXECUTE ON FUNCTION public.set_library_context(bigint, public.library_type) TO api_user;

-- Add comments for documentation
COMMENT ON POLICY items_library_isolation ON public.items IS 'Ensures users can only access items from their authorized library';
COMMENT ON POLICY collections_library_isolation ON public.collections IS 'Ensures users can only access collections from their authorized library';
COMMENT ON POLICY tags_library_isolation ON public.tags IS 'Ensures users can only access tags from their authorized library';

COMMENT ON VIEW public.secure_items_view IS 'RLS-protected view of items that automatically filters by user library access';
COMMENT ON VIEW public.secure_collections_view IS 'RLS-protected view of collections that automatically filters by user library access';
COMMENT ON VIEW public.secure_tags_view IS 'RLS-protected view of tags that automatically filters by user library access';
COMMENT ON VIEW public.secure_libraries_view IS 'RLS-protected view of libraries that automatically filters by user library access';

COMMENT ON FUNCTION public.set_library_context(bigint, public.library_type) IS 'Sets the current library context for RLS policies when JWT claims are not available'; 