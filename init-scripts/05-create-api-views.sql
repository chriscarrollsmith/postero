-- Create enhanced views for PostgREST API access

-- Enhanced items view with flattened structure
CREATE OR REPLACE VIEW public.items_view AS
SELECT
    i.key,
    i.library as group_id,
    i.version,
    i.sync,
    i.trashed,
    i.deleted,
    i.modified,
    i.gitlab,
    i.md5,
    -- Extract commonly used fields from JSON data
    i.data->>'itemType' as item_type,
    i.data->>'title' as title,
    i.data->>'url' as url,
    i.data->>'note' as note,
    i.data->>'abstractNote' as abstract,
    i.data->>'publicationTitle' as publication_title,
    i.data->>'date' as date,
    i.data->>'DOI' as doi,
    i.data->>'ISBN' as isbn,
    i.data->>'ISSN' as issn,
    i.data->>'volume' as volume,
    i.data->>'issue' as issue,
    i.data->>'pages' as pages,
    i.data->>'language' as language,
    i.data->>'parentItem' as parent_item,
    i.data->>'filename' as filename,
    i.data->>'contentType' as content_type,
    i.data->>'linkMode' as link_mode,
    -- Extract collections array
    CASE 
        WHEN jsonb_typeof(i.data->'collections') = 'array' 
        THEN ARRAY(SELECT jsonb_array_elements_text(i.data->'collections'))
        ELSE ARRAY[]::text[]
    END as collection_keys,
    -- Extract tags array
    CASE 
        WHEN jsonb_typeof(i.data->'tags') = 'array' 
        THEN ARRAY(SELECT jsonb_array_elements_text(i.data->'tags'))
        ELSE ARRAY[]::text[]
    END as tag_names,
    -- Keep full JSON data for complete access
    i.data as full_data,
    i.meta as meta_data
FROM public.items i;

-- Enhanced collections view with flattened structure
CREATE OR REPLACE VIEW public.collections_view AS
SELECT
    c.key,
    c.library as group_id,
    c.version,
    c.sync,
    c.deleted,
    c.modified,
    c.gitlab,
    -- Extract fields from JSON data
    c.data->>'name' as name,
    c.data->>'parentCollection' as parent_collection,
    -- Keep full JSON data for complete access
    c.data as full_data,
    c.meta as meta_data
FROM public.collections c;

-- Enhanced groups view
CREATE OR REPLACE VIEW public.groups_view AS
SELECT
    g.id as group_id,
    g.version,
    g.created,
    g.modified,
    g.deleted,
    g.itemversion,
    g.collectionversion,
    g.tagversion,
    g.gitlab,
    -- Extract fields from JSON data if they exist
    g.data->>'name' as name,
    g.data->>'description' as description,
    g.data->>'type' as group_type,
    g.data->>'url' as url,
    -- Keep full JSON data
    g.data as full_data
FROM public.groups g;

-- Enhanced tags view
CREATE OR REPLACE VIEW public.tags_view AS
SELECT
    t.tag as name,
    t.library as group_id,
    t.meta as meta_data
FROM public.tags t;

-- Collection lookup function
CREATE OR REPLACE FUNCTION public.get_collection_by_name(
    p_group_id bigint,
    p_name text,
    p_parent_key text DEFAULT NULL
)
RETURNS TABLE(
    key varchar(8),
    group_id bigint,
    name text,
    parent_collection text,
    version bigint,
    sync public.syncstatus,
    full_data jsonb
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        cv.key,
        cv.group_id,
        cv.name,
        cv.parent_collection,
        cv.version,
        cv.sync,
        cv.full_data
    FROM public.collections_view cv
    WHERE cv.group_id = p_group_id
      AND cv.name = p_name
      AND cv.deleted = false
      AND (p_parent_key IS NULL OR cv.parent_collection = p_parent_key);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Item lookup function by old ID (if needed for backward compatibility)
CREATE OR REPLACE FUNCTION public.get_item_by_oldid(
    p_group_id bigint,
    p_oldid text
)
RETURNS TABLE(
    key varchar(8),
    group_id bigint,
    item_type text,
    title text,
    version bigint,
    sync public.syncstatus,
    full_data jsonb
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        iv.key,
        iv.group_id,
        iv.item_type,
        iv.title,
        iv.version,
        iv.sync,
        iv.full_data
    FROM public.items_view iv
    WHERE iv.group_id = p_group_id
      AND iv.full_data->>'oldid' = p_oldid
      AND iv.deleted = false;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Function to refresh materialized views
CREATE OR REPLACE FUNCTION public.refresh_materialized_views()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY public.collection_name_hier;
    REFRESH MATERIALIZED VIEW CONCURRENTLY public.item_type_hier;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Grant permissions on views to API roles
GRANT SELECT ON public.items_view TO api_anon, api_user;
GRANT SELECT ON public.collections_view TO api_anon, api_user;
GRANT SELECT ON public.groups_view TO api_anon, api_user;
GRANT SELECT ON public.tags_view TO api_anon, api_user;

-- Grant execute permissions on functions
GRANT EXECUTE ON FUNCTION public.get_collection_by_name(bigint, text, text) TO api_anon, api_user;
GRANT EXECUTE ON FUNCTION public.get_item_by_oldid(bigint, text) TO api_anon, api_user;
GRANT EXECUTE ON FUNCTION public.refresh_materialized_views() TO api_user;

-- Add comments for API documentation
COMMENT ON VIEW public.items_view IS 'Flattened view of items with commonly used fields extracted from JSON data for easy API access';
COMMENT ON VIEW public.collections_view IS 'Flattened view of collections with commonly used fields extracted from JSON data';
COMMENT ON VIEW public.groups_view IS 'Flattened view of groups/libraries with metadata';
COMMENT ON VIEW public.tags_view IS 'Simple view of tags with group association';

COMMENT ON FUNCTION public.get_collection_by_name(bigint, text, text) IS 'Find a collection by name within a group, optionally scoped by parent collection';
COMMENT ON FUNCTION public.get_item_by_oldid(bigint, text) IS 'Find an item by its old ID for backward compatibility';
COMMENT ON FUNCTION public.refresh_materialized_views() IS 'Refresh all materialized views used by the API'; 