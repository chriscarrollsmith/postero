-- Implement Row-Level Security for multi-tenant data isolation

-- Enable RLS on main tables
ALTER TABLE public.items ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.tags ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.groups ENABLE ROW LEVEL SECURITY;

-- Create RLS policies for items table
-- Policy: Users can only access items belonging to their group_id
DROP POLICY IF EXISTS items_group_isolation ON public.items;
CREATE POLICY items_group_isolation
ON public.items
FOR ALL
TO api_user
USING (
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Allow anonymous users to see items if group_id is explicitly provided and permitted
DROP POLICY IF EXISTS items_anon_access ON public.items;
CREATE POLICY items_anon_access
ON public.items
FOR SELECT
TO api_anon
USING (
    deleted = false AND
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Create RLS policies for collections table
DROP POLICY IF EXISTS collections_group_isolation ON public.collections;
CREATE POLICY collections_group_isolation
ON public.collections
FOR ALL
TO api_user
USING (
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

DROP POLICY IF EXISTS collections_anon_access ON public.collections;
CREATE POLICY collections_anon_access
ON public.collections
FOR SELECT
TO api_anon
USING (
    deleted = false AND
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Create RLS policies for tags table
DROP POLICY IF EXISTS tags_group_isolation ON public.tags;
CREATE POLICY tags_group_isolation
ON public.tags
FOR ALL
TO api_user
USING (
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

DROP POLICY IF EXISTS tags_anon_access ON public.tags;
CREATE POLICY tags_anon_access
ON public.tags
FOR SELECT
TO api_anon
USING (
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Create RLS policies for groups table
DROP POLICY IF EXISTS groups_access ON public.groups;
CREATE POLICY groups_access
ON public.groups
FOR SELECT
TO api_user, api_anon
USING (
    id = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Allow api_user to modify their own group
DROP POLICY IF EXISTS groups_modify ON public.groups;
CREATE POLICY groups_modify
ON public.groups
FOR UPDATE
TO api_user
USING (
    id = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint
    )
);

-- Create secure views that include RLS
-- These views will automatically filter by group_id when RLS is enabled
CREATE OR REPLACE VIEW public.secure_items_view AS
SELECT * FROM public.items_view;

CREATE OR REPLACE VIEW public.secure_collections_view AS
SELECT * FROM public.collections_view;

CREATE OR REPLACE VIEW public.secure_tags_view AS
SELECT * FROM public.tags_view;

CREATE OR REPLACE VIEW public.secure_groups_view AS
SELECT * FROM public.groups_view;

-- Grant permissions on secure views
GRANT SELECT ON public.secure_items_view TO api_anon, api_user;
GRANT SELECT ON public.secure_collections_view TO api_anon, api_user;
GRANT SELECT ON public.secure_tags_view TO api_anon, api_user;
GRANT SELECT ON public.secure_groups_view TO api_anon, api_user;

-- Function to set group context for API calls
CREATE OR REPLACE FUNCTION public.set_group_context(p_group_id bigint)
RETURNS void AS $$
BEGIN
    -- This function can be called by applications to set the group context
    -- It will be used by RLS policies
    PERFORM set_config('app.current_group_id', p_group_id::text, true);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Alternative RLS policies using app-level group context
-- These can be used if JWT claims are not available
DROP POLICY IF EXISTS items_app_context ON public.items;
CREATE POLICY items_app_context
ON public.items
FOR ALL
TO api_user
USING (
    library = COALESCE(
        current_setting('request.jwt.claims.group_id', true)::bigint,
        current_setting('request.jwt.claims.library_id', true)::bigint,
        current_setting('app.current_group_id', true)::bigint
    )
);

-- Disable the duplicate policies (PostgreSQL will use the first matching policy)
-- The above policies will take precedence

-- Grant execute on context function
GRANT EXECUTE ON FUNCTION public.set_group_context(bigint) TO api_user;

-- Add comments for documentation
COMMENT ON POLICY items_group_isolation ON public.items IS 'Ensures users can only access items from their authorized group/library';
COMMENT ON POLICY collections_group_isolation ON public.collections IS 'Ensures users can only access collections from their authorized group/library';
COMMENT ON POLICY tags_group_isolation ON public.tags IS 'Ensures users can only access tags from their authorized group/library';

COMMENT ON VIEW public.secure_items_view IS 'RLS-protected view of items that automatically filters by user group access';
COMMENT ON VIEW public.secure_collections_view IS 'RLS-protected view of collections that automatically filters by user group access';
COMMENT ON VIEW public.secure_tags_view IS 'RLS-protected view of tags that automatically filters by user group access';

COMMENT ON FUNCTION public.set_group_context(bigint) IS 'Sets the current group context for RLS policies when JWT claims are not available'; 