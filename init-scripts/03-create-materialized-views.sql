-- Create materialized views for efficient querying

-- Collection hierarchy materialized view
CREATE MATERIALIZED VIEW IF NOT EXISTS public.collection_name_hier AS
WITH RECURSIVE collection_hierarchy AS (
    -- Base case: collections without parent
    SELECT 
        c.key,
        c.library,
        c.data->>'name' as name,
        c.data->>'parentCollection' as parent_key,
        c.data->>'name' as path,
        0 as level
    FROM public.collections c
    WHERE c.data->>'parentCollection' IS NULL OR c.data->>'parentCollection' = ''
    
    UNION ALL
    
    -- Recursive case: collections with parent
    SELECT 
        c.key,
        c.library,
        c.data->>'name' as name,
        c.data->>'parentCollection' as parent_key,
        ch.path || ' > ' || (c.data->>'name') as path,
        ch.level + 1 as level
    FROM public.collections c
    JOIN collection_hierarchy ch ON c.data->>'parentCollection' = ch.key AND c.library = ch.library
    WHERE c.data->>'parentCollection' IS NOT NULL AND c.data->>'parentCollection' != ''
)
SELECT 
    key,
    library,
    name,
    parent_key,
    path,
    level
FROM collection_hierarchy;

-- Item type hierarchy materialized view
CREATE MATERIALIZED VIEW IF NOT EXISTS public.item_type_hier AS
SELECT 
    i.key,
    i.library,
    i.data->>'itemType' as item_type,
    i.data->>'title' as title,
    i.data->>'parentItem' as parent_item,
    CASE 
        WHEN i.data->>'parentItem' IS NOT NULL AND i.data->>'parentItem' != '' THEN 'child'
        ELSE 'parent'
    END as hierarchy_level,
    i.data->>'collections' as collections,
    array_length(
        CASE 
            WHEN jsonb_typeof(i.data->'collections') = 'array' 
            THEN ARRAY(SELECT jsonb_array_elements_text(i.data->'collections'))
            ELSE ARRAY[]::text[]
        END, 1
    ) as collection_count
FROM public.items i
WHERE i.deleted = false;

-- Create indexes on materialized views
CREATE INDEX IF NOT EXISTS idx_collection_name_hier_library ON public.collection_name_hier(library);
CREATE INDEX IF NOT EXISTS idx_collection_name_hier_parent ON public.collection_name_hier(parent_key);
CREATE INDEX IF NOT EXISTS idx_collection_name_hier_level ON public.collection_name_hier(level);

CREATE INDEX IF NOT EXISTS idx_item_type_hier_library ON public.item_type_hier(library);
CREATE INDEX IF NOT EXISTS idx_item_type_hier_type ON public.item_type_hier(item_type);
CREATE INDEX IF NOT EXISTS idx_item_type_hier_parent ON public.item_type_hier(parent_item);
CREATE INDEX IF NOT EXISTS idx_item_type_hier_level ON public.item_type_hier(hierarchy_level);

-- Create unique indexes for concurrent refresh capability
CREATE UNIQUE INDEX IF NOT EXISTS idx_collection_name_hier_unique ON public.collection_name_hier(key, library);
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_type_hier_unique ON public.item_type_hier(key, library);

-- Initial refresh (non-concurrent since views are empty)
-- These will run only if the views were newly created or if they need refreshing.
REFRESH MATERIALIZED VIEW public.collection_name_hier;
REFRESH MATERIALIZED VIEW public.item_type_hier; 